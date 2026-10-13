-- Prints the results of gte-vz0-delay.exe and quits.
local mem = ffi.cast('uint32_t*', PCSX.getMemPtr())
local frames = 0
KEEP = PCSX.Events.createEventListener('GPU::Vsync', function()
    frames = frames + 1
    if mem[0x20000 / 4 + 4] == 0x600d then
        for n = 0, 3 do
            print(string.format('N=%d nops: MVMVA used VZ0 = 0x%x', n, mem[0x20000 / 4 + n]))
        end
        PCSX.quit(0)
    elseif frames > 3000 then
        print('timeout')
        PCSX.quit(1)
    end
end)
