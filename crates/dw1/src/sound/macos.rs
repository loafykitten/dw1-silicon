//! A modeless volume window: host VBlanks must keep game audio flowing while it is open.
use super::{Action, State};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSMenu, NSMenuItem, NSWindow, NSWindowDelegate,
    NSWindowStyleMask,
};
use objc2_foundation::{
    MainThreadMarker, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, ns_string,
};
use objc2_web_kit::{
    WKScriptMessage, WKScriptMessageHandler, WKUserContentController, WKWebView,
    WKWebViewConfiguration, WKWebsiteDataStore,
};
use sdl3::audio::AudioStreamOwner;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub struct SoundMenu {
    menu: Retained<NSMenu>,
    item: Retained<NSMenuItem>,
    target: Retained<SoundController>,
}
impl Drop for SoundMenu {
    fn drop(&mut self) {
        self.target.close(false);
        self.menu.removeItem(&self.item);
    }
}
impl SoundMenu {
    pub fn install(output: Option<Rc<AudioStreamOwner>>) -> Result<Self, String> {
        SoundController::install(output)
    }
    pub fn input_epoch(&self) -> (u64, u64) {
        (
            self.target.ivars().revision.get(),
            self.target.ivars().cutoff.get(),
        )
    }
    pub fn focused(&self) -> bool {
        self.target
            .ivars()
            .window
            .borrow()
            .as_ref()
            .is_some_and(|w| w.isKeyWindow())
    }
    pub fn open(&self) {
        self.target.open();
    }
    pub fn percent(&self) -> u8 {
        self.target.ivars().state.borrow().volume.percent()
    }
}

pub struct Ivars {
    state: RefCell<State>,
    // Owning reference, not a dangling raw stream pointer after host teardown.
    output: Option<Rc<AudioStreamOwner>>,
    window: RefCell<Option<Retained<NSWindow>>>,
    previous: RefCell<Option<Retained<NSWindow>>>,
    web: RefCell<Option<Retained<WKWebView>>>,
    revision: Cell<u64>,
    cutoff: Cell<u64>,
}
define_class!(
    // SAFETY: NSObject has no subclassing requirements; all callbacks/ivars are main-thread-only.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = Ivars]
    pub struct SoundController;
    unsafe impl NSObjectProtocol for SoundController {}
    impl SoundController {
        #[unsafe(method(openSound:))]
        fn open_sound(&self, _sender: Option<&NSObject>) { self.open(); }
    }
    unsafe impl NSWindowDelegate for SoundController {
        #[unsafe(method(windowShouldClose:))]
        fn window_should_close(&self, _window: &NSWindow) -> bool {
            self.close(true);
            // close() already orders out/releases our explicitly retained window.
            false
        }
    }
    unsafe impl WKScriptMessageHandler for SoundController {
        #[unsafe(method(userContentController:didReceiveScriptMessage:))]
        unsafe fn receive(&self, _controller: &WKUserContentController, message: &WKScriptMessage) {
            if !unsafe { message.frameInfo().isMainFrame() } { return; }
            let body = unsafe { message.body() };
            let Some(body) = body.downcast_ref::<NSString>() else { return; };
            let Some(action) = super::action(&body.to_string()) else { return; };
            match action {
                Action::Volume(n) => self.ivars().state.borrow_mut().change(n, |g| self.apply(g)),
                Action::Mute => self.ivars().state.borrow_mut().toggle(|g| self.apply(g)),
                Action::Commit => self.ivars().state.borrow_mut().commit(),
                Action::Done => { self.close(true); return; },
                Action::Ready => {},
            }
            if std::env::var_os("DW1_WINDOW_LOG").is_some() {
                let state = self.ivars().state.borrow();
                eprintln!("dw1: sound volume {}%, gain {:.2}", state.volume.percent(), state.volume.gain());
            }
            self.render();
        }
    }
);
impl SoundController {
    fn install(output: Option<Rc<AudioStreamOwner>>) -> Result<SoundMenu, String> {
        let mtm = MainThreadMarker::new().ok_or("Sound must be created on the main thread")?;
        let mut state = State::load();
        if let Some(stream) = &output {
            if let Err(e) = stream.set_gain(state.volume.gain()) {
                state.error = format!("Could not apply saved volume: {e}");
            }
        }
        let this = Self::alloc(mtm).set_ivars(Ivars {
            state: RefCell::new(state),
            output,
            window: RefCell::new(None),
            previous: RefCell::new(None),
            web: RefCell::new(None),
            revision: Cell::new(0),
            cutoff: Cell::new(0),
        });
        // SAFETY: NSObject's init signature, retained target outlives the installed item.
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };
        let menu = NSApplication::sharedApplication(mtm)
            .mainMenu()
            .ok_or("SDL did not create the macOS menu")?;
        let top = NSMenuItem::new(mtm);
        top.setTitle(ns_string!("Sound"));
        let submenu = NSMenu::initWithTitle(NSMenu::alloc(mtm), ns_string!("Sound"));
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                ns_string!("Volume…"),
                Some(sel!(openSound:)),
                ns_string!(""),
            )
        };
        unsafe { item.setTarget(Some(&this)) };
        submenu.addItem(&item);
        top.setSubmenu(Some(&submenu));
        menu.insertItem_atIndex(&top, 2.min(menu.numberOfItems()));
        Ok(SoundMenu {
            menu,
            item: top,
            target: this,
        })
    }
    fn apply(&self, gain: f32) -> Result<(), String> {
        self.ivars()
            .output
            .as_ref()
            .map_or(Ok(()), |s| s.set_gain(gain).map_err(|e| e.to_string()))
    }
    fn epoch(&self) {
        self.ivars()
            .cutoff
            .set(unsafe { sdl3::sys::timer::SDL_GetTicksNS() });
        self.ivars()
            .revision
            .set(self.ivars().revision.get().wrapping_add(1));
    }
    fn render(&self) {
        if let Some(web) = &*self.ivars().web.borrow() {
            let json = self
                .ivars()
                .state
                .borrow()
                .view(self.ivars().output.is_some());
            let script = NSString::from_str(&format!("window.renderSound({json});"));
            unsafe { web.evaluateJavaScript_completionHandler(&script, None) };
        }
    }
    fn close(&self, restore_focus: bool) {
        self.ivars().state.borrow_mut().commit();
        let web = self.ivars().web.borrow_mut().take();
        if let Some(web) = web {
            // Break the WebKit -> handler -> WebKit cycle before releasing either window.
            unsafe {
                web.configuration()
                    .userContentController()
                    .removeScriptMessageHandlerForName(ns_string!("sound"))
            };
        }
        if let Some(window) = self.ivars().window.borrow_mut().take() {
            window.setDelegate(None);
            window.orderOut(None);
            // No automatic release-on-close: the retained object is released normally.
            window.close();
            self.epoch();
        }
        let previous = self.ivars().previous.borrow_mut().take();
        if restore_focus {
            if let Some(previous) = previous {
                previous.makeKeyAndOrderFront(None);
            }
        }
    }
    fn open(&self) {
        if let Some(window) = &*self.ivars().window.borrow() {
            window.makeKeyAndOrderFront(None);
            return;
        }
        let mtm = self.mtm();
        let app = NSApplication::sharedApplication(mtm);
        *self.ivars().previous.borrow_mut() = app.keyWindow().or_else(|| app.mainWindow());
        let frame = NSRect::new(NSPoint::new(0., 0.), NSSize::new(510., 184.));
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                frame,
                NSWindowStyleMask::Titled | NSWindowStyleMask::Closable,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        unsafe { window.setReleasedWhenClosed(false) };
        window.setTitle(ns_string!("Sound"));
        window.setDelegate(Some(ProtocolObject::from_ref(self)));
        let config = unsafe { WKWebViewConfiguration::new(mtm) };
        unsafe {
            config.setWebsiteDataStore(&WKWebsiteDataStore::nonPersistentDataStore(mtm));
            config
                .preferences()
                .setJavaScriptCanOpenWindowsAutomatically(false);
            config
                .userContentController()
                .addScriptMessageHandler_name(ProtocolObject::from_ref(self), ns_string!("sound"));
        }
        let web = unsafe {
            WKWebView::initWithFrame_configuration(WKWebView::alloc(mtm), frame, &config)
        };
        window.setContentView(Some(&web));
        *self.ivars().window.borrow_mut() = Some(window.clone());
        *self.ivars().web.borrow_mut() = Some(web.clone());
        unsafe {
            web.loadHTMLString_baseURL(&NSString::from_str(include_str!("volume.html")), None);
        }
        self.epoch();
        window.center();
        window.makeKeyAndOrderFront(None);
        window.makeFirstResponder(Some(&web));
        // No runModalForWindow: host VBlanks keep feeding the existing audio stream.
    }
}
