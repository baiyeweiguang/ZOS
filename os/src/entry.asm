    .section .text.entry
    .globl _start ; _start标记为全局可见 可以被其他模块和文件引用或访问
_start:
    la sp, boot_stack_top ; la(load address)
    call rust_main

    .section .bss.stack
    .globl boot_stack_lower_bound
boot_stack_lower_bound:
    .space 4096 * 16 ; 从boot_stack_lower_bound地址开始 连续分配4096*16字节空间
    .globl boot_stack_top
boot_stack_top: