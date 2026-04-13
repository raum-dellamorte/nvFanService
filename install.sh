# Builds and Installs nvFanService to .local/bin
# might fail, idk, whatevs
echo "Running cargo update"
cargo update
echo "Building in release mode..."
cargo build -r
echo "Installing nvfanservice to /usr/bin"
sudo cp target/release/nvfanservice /usr/bin
echo "Installing nvfanservice.jpg to /usr/share/icons/hicolor/128x128"
sudo cp res/nvfanservice.png /usr/share/icons/hicolor/128x128
echo "Installing nvfanservice.desktop to /usr/share/applications"
sudo cp res/nvFanService.desktop /usr/share/applications
echo "Installing nvfanservice.service to /etc/systemd/system"
sudo cp res/nvfanservice.service /etc/systemd/system
echo "Run `sudo systemctl enable --now nvfanservice` to enable and start the service."
echo "Run nvfanservice as a user on the `wheel` group to open the GUI client."
