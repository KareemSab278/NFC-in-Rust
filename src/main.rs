mod nfc;

fn main() {
    nfc::read().expect("Failed to run NFC code");
}
