# nvFanService
### Running Wayland and can't use GreenWithEnvy? Relax. Raum brings Fan Service to your nVidia GPU.
No promises. But it just may work.

## Currently:
- Fan speeds are hard coded, but now we have a way to create and use custom fan curves... UwU
- We're printing the temp and fan speed to the terminal every 10 seconds
- It seems to work on Nobara 40 (Fedora 40 ala GloriusEggroll)
- Uses sudo crate to automatically prompt for password
  - This eases development as I can just `cargo run -r` without trying to `sudo` my `cargo`
  - The code is really short at the moment and easy to peruse as I am just some guy on the internet and trust is not safe. Trust no one. They're coming for you, Barbara.
- press 'q' then 'Enter' to quit.

## Todo:
- "GUI": Seems like a good excuse to learn ncurses or the like
- Hard coded "fan curve" is bad and the current values are for testing purposes so I can hear it working
  - [x] Made a kind of fan curve struct to pass to service_fans()... UwU
  - [x] Said struct is a sorted list of (temp, speed) tuples
  - [ ] Currently we have fixed steps. I think we want to lerp speeds between temps

Good talk...
