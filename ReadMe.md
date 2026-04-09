# nvFanService

### Running Wayland and can't use GreenWithEnvy? Relax. Raum brings Fan Service to your nVidia GPU.

No promises. But it just may work.

Default SDL3 version:

![nvFanService-sdl3-example1](nvFanService-sdl3-example1.png)

Terminal version using `ncurses` via `cursive` crate:

![nvFanService-example](nvFanService-example.png)
![nvFanService-example2](nvFanService-example2.png)

## Compiling:

On Arch, you'll need extra/sdl3 and aur/sdl3_ttf to compile. I guess sdl3 is too new for everything to be on the main repo? Patience, Iago. I assume you'll need to find the equivalent packages on other distros, perhaps even as dev packages, IDK. One day I'll find out and make specific notes.

Also, the font for the SDL3 version is hard coded to `"/usr/share/fonts/TTF/FiraCodeNerdFontMono-Regular.ttf"` and I imagine it will crash without that file. Can I... `include_bytes!()` the font file??? That would make it a compile dependency and not really solve anything, right? This needs to be dealt with somehow... Figure out the most common mono fonts, put them in a list by preference, then load the first one that exists?

## Currently:

- Update: Starting temperature/fan speed curve is read from `/etc/nvFanService/config.kdl`
  - See [KDL](https://kdl.dev/) for more information about the format
  - The file is created with default values if it doesn't exist
  - `/home/<user>/.config/nvFanService/config.kdl` is also created but not yet used
    - The next planned feature is to split the executable into a daemon and gui/tui clients to avoid running the frontend as root
  - Fan curves (UwU) are partially configurable in the gui/tui with a series of sliders
  - Number of sliders and their temperatures are not yet configurable within the gui/tui
    - You can change the fan curve by editing the `/etc/nvFanService/config.kdl` file, just pay attention to the format
    - Temps are only in Celsius and only valid between 5 and 95 degrees, for reasons
    - Temps are written `:<1 to 2 digits>C`, such as `:9C`, `:09C`, or `:55C` (the `:` is a personal choice to make it valid KDL)
    - Fan speeds are written as `@<1 to 3 digits>%`, such as `@8%`, `@08%`, `@50%`, or `@100%` (`@` for valid KDL as above)
    - Temp/Speed pairs are written separated by at least one space, such as `:36C    @60%`
      - The number of spaces is arbitrary, whatever you like
      - One pair per line
      - A correct config.kdl keeps these lines between `fan_curve {` and `}`, scroll down for an example
  - Changes to the sliders take effect by the next time the temp is polled
    - Temp is polled around every 10 seconds
  - No safety function is in place for a bad fan curve
    - A bad fan curve can potentially cause your card to overheat
    - Setting fan speed to 0%, IIRC, returns control to firmware
    - Therefore, setting speed to 1% across the board is bad
    - Default fan curve is aggressive and potentially loud, but safe(TM)
    - Exiting the program returns control to firmware
- Uses SDL3 by default but can be run in a terminal window with the command line option `cli`. This runs the Cursive/ncurses version, in which case, using the `cursive` crate, an ncurses panel is displayed in which the name of the card along with it's temp and fan speed are displayed, refreshed every 10 seconds~~
  - I can envision a future in which one can use their own color theme from a file.
- Testing: Tested recently on Arch
- Uses elevate.rs, my derivative of the `sudo` crate, to relaunch as root
  - This is going away when we split into daemon and client
  - Why root? `nvml_wrapper` is read only for temp/speed without root privileges
  - elevate.rs uses ~~`sudo` if launched from the terminal~~ `pkexec` to elevate privileges
  - This eases development as I can just `cargo run -r` without trying to `sudo` my `cargo`
  - The code is ~~really~~ short at the moment and easy to peruse, which you should do as I am just some guy on the internet and it's really quite mad to trust just some guy on the internet. Trust no one. They're coming for you, Barbara.
- press 'q' to quit. No more 'q' then 'Enter' garbage.
  - As mentioned above, control is returned to firmware when the main loop ends. It should be safe(TM). I've been running nvFanService 24/7 for days at a time when not developing new features or having some reason to restart my computer and the stability has been rock solid. Still, use at your own risk.

Current default config.kdl:
```kdl
// nvFanService settings
fan_curve {
  :10C      @0%
  :20C     @30%
  :30C     @60%
  :36C     @70%
  :40C     @80%
  :46C     @90%
  :50C    @100%
}
```

## Todo:

The SDL3 version is coming along nicely!

- [x] Load/Save fan curve values in a config file
  - read from /etc/nvFanService/config.kdl
  - [ ] Split into daemon and client so that client side changes are forwarded to the daemon
- [x] Lerp speeds between temps
- Curve Editor
  - [x] is now displayed
  - [x] sliders "slide"
    - shadow knob appears with what fan speed would be at current mouse position
    - left mouse button release sets fan speed to what is shown on the shadow knob
    - you can effectively drag the knob; only shadow knob moves until button release
  - [ ] need a client side way to change temperature per slider
  - [ ] need a client side way to add/remove sliders (within reason)
    - currently possible by editing `/etc/nvFanService/config.kdl`
  - [ ] make prettier

Good talk...
