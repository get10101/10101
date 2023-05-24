// This file is used to generate the environment variables for the Rust analyzer
// to make autometrics docstrings resolve the chosen Prometheus URL.

fn main() {
    // Uncomment the `premetheus_url` line with the desired URL
    // Note: Reload Rust analyzer after changing the Prometheus URL to regenerate the links

    // regtest URL
    let prometheus_url = "http://testnet.itchysats.network:9090";

    // local debugging
    // let prometheus_url = "http://localhost:9090";
    println!("cargo:rustc-env=PROMETHEUS_URL={prometheus_url}");
}
