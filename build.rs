fn main() {
    //println!("cargo:rerun-if-changed=libsample.so");
    println!("cargo:rustc-link-search=native=.");
    println!("cargo:rustc-link-lib=dylib=sample");
    println!("cargo:rustc-env=LD_LIBRARY_PATH=.");
}