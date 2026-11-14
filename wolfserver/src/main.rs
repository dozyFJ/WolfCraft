//! WolfServer CLI. The same core is loaded through JNI by the Android shell.
fn main(){if let Err(e)=wolfserver_runtime::run(std::env::args().skip(1).collect()){eprintln!("WolfServer: {e}");std::process::exit(1);}}
