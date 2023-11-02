run_qemu:
	qemu-system-riscv64 \
    -machine virt \
    -nographic \
    -bios /mnt/bootloader/rustsbi-qemu.bin \
    -device loader,file=target/riscv64gc-unknown-none-elf/release/os,addr=0x80200000 