# Nuke built-in rules and variables.
override MAKEFLAGS += -rR

override IMAGE_NAME := os

.PHONY: all
all: clean-img kernel $(IMAGE_NAME).iso

.PHONY: all-test
all-test: clean-img kernel-test $(IMAGE_NAME).iso

.PHONY: all-hdd
all-hdd: clean-img kernel $(IMAGE_NAME).hdd

.PHONY: all-test-hdd
all-test-hdd: clean-img kernel-test $(IMAGE_NAME).hdd

.PHONY: run
run: clean-img kernel $(IMAGE_NAME).iso
	qemu-system-x86_64 -m 4G -boot d -cdrom $(IMAGE_NAME).iso -drive file=storage.img,format=raw,media=disk
.PHONY: run-test
run-test: clean-img kernel-test $(IMAGE_NAME).iso
	qemu-system-x86_64 -m 4G -boot d -cdrom $(IMAGE_NAME).iso -drive file=storage.img,format=raw,media=disk


.PHONY: run-uefi
run-uefi: clean-img kernel ovmf $(IMAGE_NAME).iso
	qemu-system-x86_64 -m 4G -bios ovmf/OVMF.fd -boot d -cdrom $(IMAGE_NAME).iso -drive file=storage.img,format=raw,media=disk

.PHONY: run-test-uefi
run-test-uefi: clean-img kernel-test ovmf $(IMAGE_NAME).iso
	qemu-system-x86_64 -m 4G -bios ovmf/OVMF.fd -boot d -cdrom $(IMAGE_NAME).iso -drive file=storage.img,format=raw,media=disk


.PHONY: run-hdd
run-hdd: clean-img kernel $(IMAGE_NAME).hdd
	qemu-system-x86_64 -m 4G -hda $(IMAGE_NAME).hdd

.PHONY: run-test-hdd
run-test-hdd: clean-img kernel-test $(IMAGE_NAME).hdd
	qemu-system-x86_64 -m 4G -hda $(IMAGE_NAME).hdd


.PHONY: run-hdd-uefi
run-hdd-uefi: kernel ovmf $(IMAGE_NAME).hdd
	qemu-system-x86_64 -m 4G -bios ovmf/OVMF.fd -hda $(IMAGE_NAME).hdd

.PHONY: run-test-hdd-uefi
run-test-hdd-uefi: kernel-test ovmf $(IMAGE_NAME).hdd
	qemu-system-x86_64 -m 4G -bios ovmf/OVMF.fd -hda $(IMAGE_NAME).hdd


ovmf:
	mkdir -p ovmf
	cd ovmf && curl -Lo OVMF.fd https://retrage.github.io/edk2-nightly/bin/RELEASEX64_OVMF.fd

limine/limine:
	rm -rf limine
	git clone https://github.com/limine-bootloader/limine.git --branch=v7.x-binary --depth=1
	$(MAKE) -C limine

.PHONY: kernel
kernel:
	$(MAKE) -C kernel all

.PHONY: kernel-test
kernel-test:
	$(MAKE) -C kernel all-test

$(IMAGE_NAME).iso: limine/limine
	rm -rf iso_root
	mkdir -p iso_root/boot
	cp -v kernel/kernel iso_root/boot/
	mkdir -p iso_root/boot/limine
	cp -v limine.cfg limine/limine-bios.sys limine/limine-bios-cd.bin limine/limine-uefi-cd.bin iso_root/boot/limine/
	mkdir -p iso_root/EFI/BOOT
	cp -v limine/BOOTX64.EFI iso_root/EFI/BOOT/
	cp -v limine/BOOTIA32.EFI iso_root/EFI/BOOT/
	xorriso -as mkisofs -b boot/limine/limine-bios-cd.bin \
		-no-emul-boot -boot-load-size 4 -boot-info-table \
		--efi-boot boot/limine/limine-uefi-cd.bin \
		-efi-boot-part --efi-boot-image --protective-msdos-label \
		iso_root -o $(IMAGE_NAME).iso
	./limine/limine bios-install $(IMAGE_NAME).iso
	rm -rf iso_root

$(IMAGE_NAME).hdd: limine/limine
	rm -f $(IMAGE_NAME).hdd
	dd if=/dev/zero bs=1M count=0 seek=64 of=$(IMAGE_NAME).hdd
	sgdisk $(IMAGE_NAME).hdd -n 1:2048 -t 1:ef00
	./limine/limine bios-install $(IMAGE_NAME).hdd
	mformat -i $(IMAGE_NAME).hdd@@1M
	mmd -i $(IMAGE_NAME).hdd@@1M ::/EFI ::/EFI/BOOT ::/boot ::/boot/limine
	mcopy -i $(IMAGE_NAME).hdd@@1M kernel/kernel ::/boot
	mcopy -i $(IMAGE_NAME).hdd@@1M limine.cfg limine/limine-bios.sys ::/boot/limine
	mcopy -i $(IMAGE_NAME).hdd@@1M limine/BOOTX64.EFI ::/EFI/BOOT
	mcopy -i $(IMAGE_NAME).hdd@@1M limine/BOOTIA32.EFI ::/EFI/BOOT

.PHONY: clean
clean:
	rm -rf iso_root $(IMAGE_NAME).iso $(IMAGE_NAME).hdd
	$(MAKE) -C kernel clean

.PHONY: clean-img
clean-img:	
	rm -rf iso_root $(IMAGE_NAME).iso $(IMAGE_NAME).hdd

.PHONY: distclean
distclean: clean
	rm -rf limine ovmf
	$(MAKE) -C kernel distclean
