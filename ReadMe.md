# nvFanService

### Running Wayland and can't use GreenWithEnvy? Relax. Raum brings Fan Service to your nVidia GPU.

No promises. But it just may work.

And if the maddening eyes of the evil gods burn somewhere other than here, this ReadMe just may be up-to-date.

Default SDL3 version:

![nvFanService-sdl3-example1](nvFanService-sdl3-example1.png)

Terminal version using `ncurses` via `cursive` crate:

![nvFanService-example](nvFanService-example.png)
![nvFanService-example2](nvFanService-example2.png)

## Compiling:

On Arch, you'll need `extra/sdl3` and `aur/sdl3_ttf` to compile. I guess sdl3 is too new for everything to be on the main repo? Patience, Iago. I assume you'll need to find the equivalent packages on other distros, perhaps even as dev packages, IDK. One day I'll find out and make specific notes.

`FiraCodeNerdFontMono-Regular` is the dev's preferred font, but `NotoSansMono-Medium` and `DejaVuSansMono` are now tried as backups if the preferred font does not exist. Adding your preferred font to the config file is implemented but untested. I believe it's `font   /path/to/font` at the top level of the config. Number of spaces is arbitrary.

## Currently:

- 2026-04-13: Daemon/Client Split!
  - When nvfanservice is run as root it acts as a daemon/service
  - When run as a user __on the `wheel` group__:
    - Without args it runs as an SDL3 client
    - With arg `cli` it runs the ncurses client, via `cursive` crate, in the terminal
    - Ratatui client planned. ~~Soon(TM)~~ Delayed bc cursive not broken
  - Clients communicate with the daemon over a unix socket
- Starting temperature/fan speed curve is read from `/etc/nvFanService/config.kdl`
  - See [KDL](https://kdl.dev/) for more information about the format
  - The file is created with default values if it doesn't exist
  - `/home/<user>/.config/nvFanService/config.kdl` is also created but not yet used
  - Fan curves (UwU) are partially configurable in the gui~~/tui~~ with a series of sliders
  - Number of sliders and their temperatures are not yet configurable within the gui/tui
    - You can change the fan curve by editing the `/etc/nvFanService/config.kdl` file, just pay attention to the format
    - Temps are only in Celsius and only valid between 5 and 95 degrees, for reasons
    - Temps are written `:<1 to 2 digits>C`, such as `:9C`, `:09C`, or `:55C` (the `:` is a personal choice to make it valid KDL)
    - Fan speeds are written as `@<1 to 3 digits>%`, such as `@8%`, `@08%`, `@50%`, or `@100%` (`@` for valid KDL as above)
    - Temp/Speed pairs are written separated by at least one space, such as `:36C    @60%`
      - The number of spaces is arbitrary, whatever you like
      - One pair per line, indented to taste
      - A correct config.kdl keeps these lines between `fan_curve {` and `}`, scroll down for an example
  - Update: We poll twice a second now ~~Changes to the sliders in the client may take a few seconds to take effect. Temp is polled around every 2 seconds~~
  - No safety function is in place for a bad fan curve
    - A bad fan curve can potentially cause your card to overheat
    - Setting fan speed to 0% returns control to firmware
    - Therefore, setting speed to 1% across the board is bad
    - Default fan curve is aggressive and potentially loud, but safe(TM) and customizable
    - Exiting the program returns control to firmware
- Requires root to control fans. Acts as a daemon/service when run as root
  - Why root? `nvml_wrapper` is read only for temp/speed without root privileges
  - I recommend reading the code as I am just some guy on the internet and it's ill-advised to trust just some guy on the internet. Trust no one. They're coming for you, Barbara.
- In the GUI and TUI clients you can press 'q' to quit, however, this does not stop the daemon/service
  - To stop the daemon, run `sudo systemctl stop nfvanservice` in a terminal
  - Stopping the daemon returns control of the fans to the hardware. It should be safe(TM). Still, USE AT YOUR OWN RISK.
- Testing: Tested recently on Arch. Again, AT YOUR OWN RISK. I've been running it as a service for weeks now and have yet to notice a problem. When I reboot, something takes a minute and a half to stop what it's doing, but I'm pretty sure that was happening before and I've been too lazy to interogate journalctl to find out just what in tarnation is a-goin' on.

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
  - [ ] Add button or KB shortcut to save client side changes to the daemon
  - [ ] Detect differences between `/etc/nvFanService/config.kdl` and `~/.config/nvFanService/config.kdl` and ask which to keep.
  - [ ] Support multiple curves, UwU
- [x] Lerp speeds between temps
- Curve Editor
  - [x] is now displayed
  - [x] SDL3: sliders "slide"
    - shadow knob appears with what fan speed would be at current mouse position
    - left mouse button release sets fan speed to what is shown on the shadow knob
    - you can effectively drag the knob; only shadow knob moves until button release
  - [ ] need a client side way to change temperature per slider
  - [ ] need a client side way to add/remove sliders (within reason)
    - currently possible by editing `/etc/nvFanService/config.kdl`
  - [ ] make prettier

Good talk...
