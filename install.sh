# Builds and Installs nvFanService to .local/bin
# might fail, idk, whatevs
cargo update
cargo build -r
cp res/nvfanservice.png ~/.local/share/icons
cp res/nvFanService.desktop ~/.local/share/applications
cp target/release/nvfanservice ~/.local/bin
