use crate::sbi::shutdown;
use core::panic::PanicInfo;

#[panic_handler]
// ! 做为返回类型时，被称为never类型，表示函数永远不会返回。
fn panic(info: &PanicInfo) -> ! {
  if let Some(location) = info.location() {
    println!(
      "Picked at {}: {} {}",
      location.file(), 
      location.line(),
      info.message()
    );
  } else {
    println!("Panicked: {}", info.message())
  }
  shutdown(true);
}