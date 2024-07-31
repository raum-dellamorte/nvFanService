# nvFanService
### Running Wayland and can't use GreenWithEnvy? Relax. Raum brings Fan Service to your nVidia GPU.
No promises. But it just may work.

## Currently:
- Fan speeds are hard coded
- We're printing the temp and fan speed to the terminal every 10 seconds
- It seems to work on Nobara 40 (Fedora 40 ala GloriusEggroll)
- press 'q' then 'Enter' to quit.

## Todo:
- "GUI": Seems like a good excuse to learn ncurses or the like
- Hard coded "fan curve" is bad and the current values are for testing purposes so I can hear it working
  - Probably make some kind of fan curve struct to pass to service_fans()
  - How hard would variable points of precision be? Keep a list of (temp, speed) tuples?
  - Do we want fixed steps? Or slide along a curve?

Good talk...
