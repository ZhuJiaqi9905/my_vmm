use std::string;

use kvm_bindings::kvm_userspace_memory_region;
use kvm_ioctls::Kvm;
use kvm_ioctls::VcpuExit;
use kvm_ioctls::VcpuFd;
use kvm_ioctls::VmFd;
use vm_memory::Address;
use vm_memory::Bytes;
use vm_memory::GuestAddress;
use vm_memory::GuestMemory;
use vm_memory::GuestMemoryMmap;
use vm_memory::GuestMemoryRegion;
use vm_memory::GuestRegionMmap;

/// 利用KVM创建一个VM。
fn main() {
    // open /dev/kvm
    let kvm = Kvm::new().unwrap();

    // assert the kvm version
    assert_eq!(kvm.get_api_version(), 12);
    print_kvm_info(&kvm);

    // create vm
    let vm = kvm.create_vm().unwrap();

    // mmap memory and write the code data to memory

    // 把%al和%bl中的整数相加，并以ASCII的形式输出。需要保证%al和%bl的和小于10。
    static data_buf: [u8; 12] = [
        0xba, 0xf8, 0x03, /* mov $0x3f8, %dx */
        0x00, 0xd8, /* add %bl, %al */
        0x04, b'0', /* add $'0', %al */
        0xee, /* out %al, (%dx) */
        0xb0, b'\n', /* mov $'\n', %al */
        0xee,  /* out %al, (%dx) */
        0xf4,  /* hlt */
    ];

    let guest_mem = GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0x1000), 0x1000)]).unwrap();
    assert_eq!(
        guest_mem.write(&data_buf, GuestAddress(0x1000)).unwrap(),
        data_buf.len()
    );
    guest_mem.iter().enumerate().for_each(|(index, region)| {
        unsafe {
            vm.set_user_memory_region(kvm_userspace_memory_region {
                slot: index as u32,
                flags: 0,
                guest_phys_addr: region.start_addr().raw_value(),
                memory_size: region.len(),
                userspace_addr: guest_mem.get_host_address(region.start_addr()).unwrap() as u64,
            })
            .unwrap();
        };
    });

    // create vcpu
    let vcpu = vm.create_vcpu(0).unwrap();
    // set registers
    let mut vcpu_sregs = vcpu.get_sregs().unwrap();
    vcpu_sregs.cs.base = 0;
    vcpu_sregs.cs.selector = 0;
    vcpu.set_sregs(&vcpu_sregs).unwrap();

    let mut vcpu_regs = vcpu.get_regs().unwrap();
    // Set the Instruction Pointer to the guest address where we loaded the code.
    vcpu_regs.rip = 0x1000;
    vcpu_regs.rax = 2;
    vcpu_regs.rbx = 2;
    vcpu_regs.rflags = 2;
    vcpu.set_regs(&vcpu_regs).unwrap();
    println!("---run vm---");
    // run VM
    loop {
        match vcpu.run().expect("run failed") {
            VcpuExit::Hlt => {
                println!("vm hlt");
                break;
            }
            VcpuExit::IoOut(port, data) => {
                if port == 0x3f8 {
                    for ch in data {
                        print!("{}", char::from_u32(*ch as u32).unwrap());
                    }
                }
            }
            exit_reason => panic!("unexpected exit reason: {:?}", exit_reason),
        }
    }
}
fn print_kvm_info(kvm: &Kvm) {
    println!(
        "the required mmap size of vcpu is: {}",
        kvm.get_vcpu_mmap_size().unwrap()
    );
    println!("max vcpu is: {}", kvm.get_max_vcpus());
}
