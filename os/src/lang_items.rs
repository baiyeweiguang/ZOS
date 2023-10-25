use core::panic::PanicInfo;

#[panic_handler]
// ! 做为返回类型时，被称为never类型，表示函数永远不会返回。
fn panic(_info: &PanicInfo) -> ! {
  // dead loop
  loop {}
}