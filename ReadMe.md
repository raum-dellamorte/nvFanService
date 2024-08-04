# nvFanService
### Running Wayland and can't use GreenWithEnvy? Relax. Raum brings Fan Service to your nVidia GPU.
No promises. But it just may work.

![nvFanService-example](nvFanService-example.png)

## Currently:
- Fan speeds are hard coded, but now we have a way to create and use custom fan curves... UwU
- We're using Cursive to create an ncurses panel where we display the temp and fan speed, refreshed every 10 seconds, and I can invision a future in which one can use their own theme from a file.
- Testing: It seems to work on Nobara 40 (Fedora 40 ala GloriusEggroll)
- Uses sudo crate to automatically prompt for password
  - This eases development as I can just `cargo run -r` without trying to `sudo` my `cargo`
  - The code is really short at the moment and easy to peruse as I am just some guy on the internet and it's really quite mad to trust just some guy on the internet. Trust no one. They're coming for you, Barbara.
- press 'q' to quit. No more 'q' then 'Enter' gargage.
  - Side note, it's possible we should be setting the fans back to default on exit and I have not yet looked into this.  I don't know if manually set speeds have a timeout or if I set it to 30% manually it just stays at 30% until told otherwise. So... use at your own risk and whatnot. I've been running the update loop for a couple of days with no instability, and when BG3 gets going, those fans go right to the top. If you prefer the quiet, my hard coded settings may not be for you. I put almost no thought into them.

## Todo:
- "GUI": Cursive TUI seems viable. Need to add fan curve customization.
- Hard coded "fan curve" is bad and the current values are for testing purposes so I can hear it working
  - [x] Made a kind of fan curve struct to pass to service_fans()... UwU
  - [x] Said struct is a sorted list of (temp, speed) tuples
  - [ ] Currently we have fixed steps. I think we want to lerp speeds between temps

Good talk...
