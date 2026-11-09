//! Main-thread Cocoa menu and local-only WebKit editor. The modal loop pauses the game
//! without advancing a single emulated field; no callback borrows the game host.
use super::EditorState;
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
use std::{cell::RefCell, rc::Rc};

pub type SharedState = Rc<RefCell<EditorState>>;

/// Removes the menu before releasing its non-owning Objective-C target.
pub struct ControlsMenu {
    menu: Retained<NSMenu>,
    item: Retained<NSMenuItem>,
    _target: Retained<MenuController>,
}
impl Drop for ControlsMenu {
    fn drop(&mut self) {
        self.menu.removeItem(&self.item);
    }
}

pub struct Ivars {
    state: SharedState,
    window: RefCell<Option<Retained<NSWindow>>>,
    web: RefCell<Option<Retained<WKWebView>>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements. Main-thread-only UI and ivars.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = Ivars]
    pub struct MenuController;

    unsafe impl NSObjectProtocol for MenuController {}
    impl MenuController {
        #[unsafe(method(openControls:))]
        fn open_controls(&self, _sender: Option<&NSObject>) { self.open(); }
    }
    unsafe impl NSWindowDelegate for MenuController {
        #[unsafe(method(windowShouldClose:))]
        fn window_should_close(&self, _window: &NSWindow) -> bool {
            NSApplication::sharedApplication(self.mtm()).stopModal();
            true
        }
    }
    unsafe impl WKScriptMessageHandler for MenuController {
        #[unsafe(method(userContentController:didReceiveScriptMessage:))]
        unsafe fn receive(&self, _controller: &WKUserContentController, message: &WKScriptMessage) {
            // Only this locally embedded document's main frame has the bridge.
            if !unsafe { message.frameInfo().isMainFrame() } { return; }
            let body = unsafe { message.body() };
            let Some(body) = body.downcast_ref::<NSString>() else { return; };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&body.to_string()) else { return; };
            match v["action"].as_str() {
                Some("bind") => {
                    if let (Some(button), Some(code)) = (v["button"].as_str(), v["code"].as_str()) {
                        self.ivars().state.borrow_mut().change(button, code);
                    }
                },
                Some("reset") => self.ivars().state.borrow_mut().reset(),
                Some("done") => {
                    if let Some(window) = &*self.ivars().window.borrow() { window.performClose(None); }
                    return;
                },
                Some("ready") => {},
                _ => return,
            }
            self.render();
        }
    }
);

impl MenuController {
    pub fn install(state: SharedState) -> Result<ControlsMenu, String> {
        let mtm = MainThreadMarker::new().ok_or("Controls must be created on the main thread")?;
        let this = Self::alloc(mtm).set_ivars(Ivars {
            state,
            window: RefCell::new(None),
            web: RefCell::new(None),
        });
        // SAFETY: NSObject's init signature. The menu target is retained by Window.
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };
        let app = NSApplication::sharedApplication(mtm);
        let menu = app.mainMenu().ok_or("SDL did not create the macOS menu")?;
        let top = NSMenuItem::new(mtm);
        top.setTitle(ns_string!("Controls"));
        let submenu = NSMenu::initWithTitle(NSMenu::alloc(mtm), ns_string!("Controls"));
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                ns_string!("Keyboard Mapping…"),
                Some(sel!(openControls:)),
                ns_string!(""),
            )
        };
        // SAFETY: Selector is implemented above, and `this` outlives the installed item.
        unsafe { item.setTarget(Some(&this)) };
        submenu.addItem(&item);
        top.setSubmenu(Some(&submenu));
        menu.insertItem_atIndex(&top, 1.min(menu.numberOfItems()));
        Ok(ControlsMenu {
            menu,
            item: top,
            _target: this,
        })
    }
    fn render(&self) {
        if let Some(web) = &*self.ivars().web.borrow() {
            let json = self.ivars().state.borrow().view();
            // Only serialized data, never interpolated executable user input or HTML.
            let script = NSString::from_str(&format!("window.renderControls({json});"));
            unsafe { web.evaluateJavaScript_completionHandler(&script, None) };
        }
    }
    pub fn open(&self) {
        if self.ivars().window.borrow().is_some() {
            return;
        }
        let mtm = self.mtm();
        let app = NSApplication::sharedApplication(mtm);
        let previous = app.keyWindow().or_else(|| app.mainWindow());
        let frame = NSRect::new(NSPoint::new(0., 0.), NSSize::new(1060., 760.));
        // SAFETY: Cocoa UI on the main thread, correctly typed initializers and retained
        // objects. NSWindow's automatic release-on-close is disabled immediately.
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
        window.setTitle(ns_string!("Keyboard Controls"));
        window.setDelegate(Some(ProtocolObject::from_ref(self)));
        let config = unsafe { WKWebViewConfiguration::new(mtm) };
        unsafe {
            config.setWebsiteDataStore(&WKWebsiteDataStore::nonPersistentDataStore(mtm));
            config
                .preferences()
                .setJavaScriptCanOpenWindowsAutomatically(false);
            config.userContentController().addScriptMessageHandler_name(
                ProtocolObject::from_ref(self),
                ns_string!("controls"),
            );
        }
        let web = unsafe {
            WKWebView::initWithFrame_configuration(WKWebView::alloc(mtm), frame, &config)
        };
        window.setContentView(Some(&web));
        *self.ivars().window.borrow_mut() = Some(window.clone());
        *self.ivars().web.borrow_mut() = Some(web.clone());
        unsafe {
            web.loadHTMLString_baseURL(&NSString::from_str(include_str!("editor.html")), None);
        }
        window.center();
        window.makeKeyAndOrderFront(None);
        app.runModalForWindow(&window);
        window.orderOut(None);
        window.setDelegate(None);
        // Break WebKit -> handler -> WebKit retention before releasing the window.
        unsafe {
            config
                .userContentController()
                .removeScriptMessageHandlerForName(ns_string!("controls"))
        };
        self.ivars().web.borrow_mut().take();
        self.ivars().window.borrow_mut().take();
        {
            let mut state = self.ivars().state.borrow_mut();
            // Same clock and units as SDL event timestamps. Do this before restoring the
            // game window, so its very first fresh key press is eligible in this same poll.
            state.closed_at_ns = unsafe { sdl3::sys::timer::SDL_GetTicksNS() };
            state.revision += 1;
        }
        if let Some(previous) = previous {
            previous.makeKeyAndOrderFront(None);
        }
    }
}
