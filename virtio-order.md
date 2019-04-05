How can we reorder virtio devices in QEMU?

# How to inspect the virtio mapping in a guest VM

    $ mount -t sysfs sys /sys      # only needed if using init=/bin/bash

    $ ls /sys/devices/platform/*.virtio_mmio
    /sys/devices/platform/10001000.virtio_mmio:
    driver_override  modalias  of_node  subsystem  uevent

    /sys/devices/platform/10002000.virtio_mmio:
    driver_override  modalias  of_node  subsystem  uevent

    /sys/devices/platform/10003000.virtio_mmio:
    driver_override  modalias  of_node  subsystem  uevent

    /sys/devices/platform/10004000.virtio_mmio:
    driver_override  modalias  of_node  subsystem  uevent

    /sys/devices/platform/10005000.virtio_mmio:
    driver_override  modalias  of_node  subsystem  uevent

    /sys/devices/platform/10006000.virtio_mmio:
    driver	driver_override  modalias  of_node  subsystem  uevent  virtio2

    /sys/devices/platform/10007000.virtio_mmio:
    driver	driver_override  modalias  of_node  subsystem  uevent  virtio1

    /sys/devices/platform/10008000.virtio_mmio:
    driver	driver_override  modalias  of_node  subsystem  uevent  virtio0

    $ ls -l /sys/devices/platform/*.virtio_mmio/virtio*/driver
    lrwxrwxrwx 1 root root 0 Jan  1 00:02 /sys/devices/platform/10006000.virtio_mmio/virtio2/driver -> ../../../../bus/virtio/drivers/virtio_net
    lrwxrwxrwx 1 root root 0 Jan  1 00:02 /sys/devices/platform/10007000.virtio_mmio/virtio1/driver -> ../../../../bus/virtio/drivers/virtio_blk

# Inspecting the QEMU device tree

Hit C-a C to enter the monitor.

Now let's run the command to inspect the device tree:

    (qemu) info qtree
    bus: main-system-bus
      type System
      [ ... ]
      dev: virtio-mmio, id ""
        [ ... ]
        mmio 0000000010008000/0000000000000200
        bus: virtio-mmio-bus.7
          type virtio-mmio-bus
          dev: virtio-rng-device, id ""
            [ ... ]
      dev: virtio-mmio, id ""
        [ ... ]
        mmio 0000000010007000/0000000000000200
        bus: virtio-mmio-bus.6
          type virtio-mmio-bus
          dev: virtio-blk-device, id ""
            [ ... ]
      dev: virtio-mmio, id ""
        [ ... ]
        mmio 0000000010006000/0000000000000200
        bus: virtio-mmio-bus.5
          type virtio-mmio-bus
          dev: virtio-net-device, id ""
            [ ... ]
      dev: virtio-mmio, id ""
        [ ... ]
        mmio 0000000010005000/0000000000000200
        bus: virtio-mmio-bus.4
          type virtio-mmio-bus
      dev: virtio-mmio, id ""
        [ ... ]
        mmio 0000000010004000/0000000000000200
        bus: virtio-mmio-bus.3
          type virtio-mmio-bus
      dev: virtio-mmio, id ""
        [ ... ]
        mmio 0000000010003000/0000000000000200
        bus: virtio-mmio-bus.2
          type virtio-mmio-bus
      dev: virtio-mmio, id ""
        [ ... ]
        mmio 0000000010002000/0000000000000200
        bus: virtio-mmio-bus.1
          type virtio-mmio-bus
      dev: virtio-mmio, id ""
        [ ... ]
        mmio 0000000010001000/0000000000000200
        bus: virtio-mmio-bus.0
          type virtio-mmio-bus
      [ ... ]

Note that the virtio-mmio-bus devices are in reverse order. This means that the command line entries are actually being
placed into memory in the order from highest address to lowest address. The linux kernel considers higher-address
devices as deserving lower numbers, so the first specified device will be virtio0, the second will be virtio1, and so
on.

Note that virtioN devices are numbered by order, and not by slot. If they were in the last three slots instead of the
first three, they'd show up with the same names as far as Linux userland cares. (Except for the differing MMIO bus
addresses in sysfs.)

# How to map virtio devices

The previously-shown mapping is for the following QEMU configuration:

    -object rng-random,filename=/dev/urandom,id=rng0
    -device virtio-rng-device,rng=rng0
    -device virtio-blk-device,drive=hd0
    -drive file=stage4-disk.img,format=raw,id=hd0
    -device virtio-net-device,netdev=usernet
    -netdev user,id=usernet,hostfwd=tcp::10000-:22

We can force which virtio device each corresponds to by including a bus=virtio-mmio-bus.N property, corresponding to the
names in the device tree shown previously. For example (not including the other required parameters):

    -device virtio-rng-device,rng=rng0,bus=virtio-mmio-bus.6
    -device virtio-blk-device,drive=hd0,bus=virtio-mmio-bus.1
    -device virtio-net-device,netdev=usernet,bus=virtio-mmio-bus.3

This configuration would yield the following device tree:

    (qemu) info qtree
    bus: main-system-bus
      type System
      [ ... ]
      dev: virtio-mmio, id ""
        [ ... ]
        mmio 0000000010008000/0000000000000200
        bus: virtio-mmio-bus.7
          type virtio-mmio-bus
      dev: virtio-mmio, id ""
        [ ... ]
        mmio 0000000010007000/0000000000000200
        bus: virtio-mmio-bus.6
          type virtio-mmio-bus
          dev: virtio-rng-device, id ""
            [ ... ]
      dev: virtio-mmio, id ""
        [ ... ]
        mmio 0000000010006000/0000000000000200
        bus: virtio-mmio-bus.5
          type virtio-mmio-bus
      dev: virtio-mmio, id ""
        [ ... ]
        mmio 0000000010005000/0000000000000200
        bus: virtio-mmio-bus.4
          type virtio-mmio-bus
      dev: virtio-mmio, id ""
        [ ... ]
        mmio 0000000010004000/0000000000000200
        bus: virtio-mmio-bus.3
          type virtio-mmio-bus
          dev: virtio-net-device, id ""
            [ ... ]
      dev: virtio-mmio, id ""
        [ ... ]
        mmio 0000000010003000/0000000000000200
        bus: virtio-mmio-bus.2
          type virtio-mmio-bus
      dev: virtio-mmio, id ""
        [ ... ]
        mmio 0000000010002000/0000000000000200
        bus: virtio-mmio-bus.1
          type virtio-mmio-bus
          dev: virtio-blk-device, id ""
            [ ... ]
      dev: virtio-mmio, id ""
        [ ... ]
        mmio 0000000010001000/0000000000000200
        bus: virtio-mmio-bus.0
          type virtio-mmio-bus
      [ ... ]

Note how the positions of the virtio devices have shifted to new buses in accordance with our configuration.

Similarly, we can observe how this looks from Linux:

    $ ls /sys/devices/platform/*.virtio_mmio
    /sys/devices/platform/10001000.virtio_mmio:
    driver_override  modalias  of_node  subsystem  uevent

    /sys/devices/platform/10002000.virtio_mmio:
    driver	driver_override  modalias  of_node  subsystem  uevent  virtio2

    /sys/devices/platform/10003000.virtio_mmio:
    driver_override  modalias  of_node  subsystem  uevent

    /sys/devices/platform/10004000.virtio_mmio:
    driver	driver_override  modalias  of_node  subsystem  uevent  virtio1

    /sys/devices/platform/10005000.virtio_mmio:
    driver_override  modalias  of_node  subsystem  uevent

    /sys/devices/platform/10006000.virtio_mmio:
    driver_override  modalias  of_node  subsystem  uevent

    /sys/devices/platform/10007000.virtio_mmio:
    driver	driver_override  modalias  of_node  subsystem  uevent  virtio0

    /sys/devices/platform/10008000.virtio_mmio:
    driver_override  modalias  of_node  subsystem  uevent

    $ ls -l /sys/devices/platform/*.virtio_mmio/virtio*/driver
    lrwxrwxrwx 1 root root 0 Jan  1 00:00 /sys/devices/platform/10002000.virtio_mmio/virtio2/driver -> ../../../../bus/virtio/drivers/virtio_blk
    lrwxrwxrwx 1 root root 0 Jan  1 00:00 /sys/devices/platform/10004000.virtio_mmio/virtio1/driver -> ../../../../bus/virtio/drivers/virtio_net

Note how the slots changed, and the ordering changed, but the devices are still numbered virtio0, virtio1, and virtio2.
(Although which device is which of virtio0, virtio1, and virtio2 has indeed changed.)
