
// References:
//
// https://github.com/qemu/qemu/blob/d522fba24478474911b0e6e488b6d1dcf1af54f8/hw/net/cadence_gem.c
// https://github.com/torvalds/linux/blob/master/drivers/net/ethernet/cadence/macb_main.c
// https://www.yumpu.com/en/document/view/31739994/gigabit-ethernet-mac-gem-technical-data-sheet-cadence-

use arr_macro::arr;
use crate::memory_region::MemoryRegion;
use crate::{pmap, riscv};
use super::*;

// Values taken for the QEMU source code.
#[allow(unused)]
mod constants {
    pub const GEM_NWCTRL: u64 = 0x00000000; /* Network Control reg */
    pub const GEM_NWCFG: u64 = 0x00000004; /* Network Config reg */
    pub const GEM_NWSTATUS: u64 = 0x00000008; /* Network Status reg */
    pub const GEM_USERIO: u64 = 0x0000000C; /* User IO reg */
    pub const GEM_DMACFG: u64 = 0x00000010; /* DMA Control reg */
    pub const GEM_TXSTATUS: u64 = 0x00000014; /* TX Status reg */
    pub const GEM_RXQBASE: u64 = 0x00000018; /* RX Q Base address reg */
    pub const GEM_TXQBASE: u64 = 0x0000001C; /* TX Q Base address reg */
    pub const GEM_RXSTATUS: u64 = 0x00000020; /* RX Status reg */
    pub const GEM_ISR: u64 = 0x00000024; /* Interrupt Status reg */
    pub const GEM_IER: u64 = 0x00000028; /* Interrupt Enable reg */
    pub const GEM_IDR: u64 = 0x0000002C; /* Interrupt Disable reg */
    pub const GEM_IMR: u64 = 0x00000030; /* Interrupt Mask reg */
    pub const GEM_PHYMNTNC: u64 = 0x00000034; /* Phy Maintenance reg */
    pub const GEM_RXPAUSE: u64 = 0x00000038; /* RX Pause Time reg */
    pub const GEM_TXPAUSE: u64 = 0x0000003C; /* TX Pause Time reg */
    pub const GEM_TXPARTIALSF: u64 = 0x00000040; /* TX Partial Store and Forward */
    pub const GEM_RXPARTIALSF: u64 = 0x00000044; /* RX Partial Store and Forward */
    pub const GEM_HASHLO: u64 = 0x00000080; /* Hash Low address reg */
    pub const GEM_HASHHI: u64 = 0x00000084; /* Hash High address reg */
    pub const GEM_SPADDR1LO: u64 = 0x00000088; /* Specific addr 1 low reg */
    pub const GEM_SPADDR1HI: u64 = 0x0000008C; /* Specific addr 1 high reg */
    pub const GEM_SPADDR2LO: u64 = 0x00000090; /* Specific addr 2 low reg */
    pub const GEM_SPADDR2HI: u64 = 0x00000094; /* Specific addr 2 high reg */
    pub const GEM_SPADDR3LO: u64 = 0x00000098; /* Specific addr 3 low reg */
    pub const GEM_SPADDR3HI: u64 = 0x0000009C; /* Specific addr 3 high reg */
    pub const GEM_SPADDR4LO: u64 = 0x000000A0; /* Specific addr 4 low reg */
    pub const GEM_SPADDR4HI: u64 = 0x000000A4; /* Specific addr 4 high reg */
    pub const GEM_TIDMATCH1: u64 = 0x000000A8; /* Type ID1 Match reg */
    pub const GEM_TIDMATCH2: u64 = 0x000000AC; /* Type ID2 Match reg */
    pub const GEM_TIDMATCH3: u64 = 0x000000B0; /* Type ID3 Match reg */
    pub const GEM_TIDMATCH4: u64 = 0x000000B4; /* Type ID4 Match reg */
    pub const GEM_WOLAN: u64 = 0x000000B8; /* Wake on LAN reg */
    pub const GEM_IPGSTRETCH: u64 = 0x000000BC; /* IPG Stretch reg */
    pub const GEM_SVLAN: u64 = 0x000000C0; /* Stacked VLAN reg */
    pub const GEM_MODID: u64 = 0x000000FC; /* Module ID reg */
    pub const GEM_OCTTXLO: u64 = 0x00000100; /* Octects transmitted Low reg */
    pub const GEM_OCTTXHI: u64 = 0x00000104; /* Octects transmitted High reg */
    pub const GEM_TXCNT: u64 = 0x00000108; /* Error-free Frames transmitted */
    pub const GEM_TXBCNT: u64 = 0x0000010C; /* Error-free Broadcast Frames */
    pub const GEM_TXMCNT: u64 = 0x00000110; /* Error-free Multicast Frame */
    pub const GEM_TXPAUSECNT: u64 = 0x00000114; /* Pause Frames Transmitted */
    pub const GEM_TX64CNT: u64 = 0x00000118; /* Error-free 64 TX */
    pub const GEM_TX65CNT: u64 = 0x0000011C; /* Error-free 65-127 TX */
    pub const GEM_TX128CNT: u64 = 0x00000120; /* Error-free 128-255 TX */
    pub const GEM_TX256CNT: u64 = 0x00000124; /* Error-free 256-511 */
    pub const GEM_TX512CNT: u64 = 0x00000128; /* Error-free 512-1023 TX */
    pub const GEM_TX1024CNT: u64 = 0x0000012C; /* Error-free 1024-1518 TX */
    pub const GEM_TX1519CNT: u64 = 0x00000130; /* Error-free larger than 1519 TX */
    pub const GEM_TXURUNCNT: u64 = 0x00000134; /* TX under run error counter */
    pub const GEM_SINGLECOLLCNT: u64 = 0x00000138; /* Single Collision Frames */
    pub const GEM_MULTCOLLCNT: u64 = 0x0000013C; /* Multiple Collision Frames */
    pub const GEM_EXCESSCOLLCNT: u64 = 0x00000140; /* Excessive Collision Frames */
    pub const GEM_LATECOLLCNT: u64 = 0x00000144; /* Late Collision Frames */
    pub const GEM_DEFERTXCNT: u64 = 0x00000148; /* Deferred Transmission Frames */
    pub const GEM_CSENSECNT: u64 = 0x0000014C; /* Carrier Sense Error Counter */
    pub const GEM_OCTRXLO: u64 = 0x00000150; /* Octects Received register Low */
    pub const GEM_OCTRXHI: u64 = 0x00000154; /* Octects Received register High */
    pub const GEM_RXCNT: u64 = 0x00000158; /* Error-free Frames Received */
    pub const GEM_RXBROADCNT: u64 = 0x0000015C; /* Error-free Broadcast Frames RX */
    pub const GEM_RXMULTICNT: u64 = 0x00000160; /* Error-free Multicast Frames RX */
    pub const GEM_RXPAUSECNT: u64 = 0x00000164; /* Pause Frames Received Counter */
    pub const GEM_RX64CNT: u64 = 0x00000168; /* Error-free 64 byte Frames RX */
    pub const GEM_RX65CNT: u64 = 0x0000016C; /* Error-free 65-127B Frames RX */
    pub const GEM_RX128CNT: u64 = 0x00000170; /* Error-free 128-255B Frames RX */
    pub const GEM_RX256CNT: u64 = 0x00000174; /* Error-free 256-512B Frames RX */
    pub const GEM_RX512CNT: u64 = 0x00000178; /* Error-free 512-1023B Frames RX */
    pub const GEM_RX1024CNT: u64 = 0x0000017C; /* Error-free 1024-1518B Frames RX */
    pub const GEM_RX1519CNT: u64 = 0x00000180; /* Error-free 1519-max Frames RX */
    pub const GEM_RXUNDERCNT: u64 = 0x00000184; /* Undersize Frames Received */
    pub const GEM_RXOVERCNT: u64 = 0x00000188; /* Oversize Frames Received */
    pub const GEM_RXJABCNT: u64 = 0x0000018C; /* Jabbers Received Counter */
    pub const GEM_RXFCSCNT: u64 = 0x00000190; /* Frame Check seq. Error Counter */
    pub const GEM_RXLENERRCNT: u64 = 0x00000194; /* Length Field Error Counter */
    pub const GEM_RXSYMERRCNT: u64 = 0x00000198; /* Symbol Error Counter */
    pub const GEM_RXALIGNERRCNT: u64 = 0x0000019C; /* Alignment Error Counter */
    pub const GEM_RXRSCERRCNT: u64 = 0x000001A0; /* Receive Resource Error Counter */
    pub const GEM_RXORUNCNT: u64 = 0x000001A4; /* Receive Overrun Counter */
    pub const GEM_RXIPCSERRCNT: u64 = 0x000001A8; /* IP header Checksum Error Counter */
    pub const GEM_RXTCPCCNT: u64 = 0x000001AC; /* TCP Checksum Error Counter */
    pub const GEM_RXUDPCCNT: u64 = 0x000001B0; /* UDP Checksum Error Counter */

    pub const GEM_1588S: u64 = 0x000001D0; /* 1588 Timer Seconds */
    pub const GEM_1588NS: u64 = 0x000001D4; /* 1588 Timer Nanoseconds */
    pub const GEM_1588ADJ: u64 = 0x000001D8; /* 1588 Timer Adjust */
    pub const GEM_1588INC: u64 = 0x000001DC; /* 1588 Timer Increment */
    pub const GEM_PTPETXS: u64 = 0x000001E0; /* PTP Event Frame Transmitted (s) */
    pub const GEM_PTPETXNS: u64 = 0x000001E4; /* PTP Event Frame Transmitted (ns) */
    pub const GEM_PTPERXS: u64 = 0x000001E8; /* PTP Event Frame Received (s) */
    pub const GEM_PTPERXNS: u64 = 0x000001EC; /* PTP Event Frame Received (ns) */
    pub const GEM_PTPPTXS: u64 = 0x000001E0; /* PTP Peer Frame Transmitted (s) */
    pub const GEM_PTPPTXNS: u64 = 0x000001E4; /* PTP Peer Frame Transmitted (ns) */
    pub const GEM_PTPPRXS: u64 = 0x000001E8; /* PTP Peer Frame Received (s) */
    pub const GEM_PTPPRXNS: u64 = 0x000001EC; /* PTP Peer Frame Received (ns) */

    /* Design Configuration Registers */
    pub const GEM_DESCONF: u64 = 0x00000280;
    pub const GEM_DESCONF2: u64 = 0x00000284;
    pub const GEM_DESCONF3: u64 = 0x00000288;
    pub const GEM_DESCONF4: u64 = 0x0000028C;
    pub const GEM_DESCONF5: u64 = 0x00000290;
    pub const GEM_DESCONF6: u64 = 0x00000294;
    pub const GEM_DESCONF6_64B_MASK: u64 = 1 << 23;
    pub const GEM_DESCONF7: u64 = 0x00000298;

    pub const GEM_INT_Q1_STATUS: u64 = 0x00000400;

    pub const GEM_TRANSMIT_Q1_PTR: u64 = 0x00000440;
    pub const GEM_TRANSMIT_Q7_PTR: u64 = GEM_TRANSMIT_Q1_PTR + 6;

    pub const GEM_RECEIVE_Q1_PTR: u64 = 0x00000480;
    pub const GEM_RECEIVE_Q7_PTR: u64 = GEM_RECEIVE_Q1_PTR + 6;

    pub const GEM_TBQPH: u64 = 0x000004C8;
    pub const GEM_RBQPH: u64 = 0x000004D4;

    pub const GEM_INT_Q1_ENABLE: u64 = 0x00000600;
    pub const GEM_INT_Q7_ENABLE: u64 = GEM_INT_Q1_ENABLE + 6;

    pub const GEM_INT_Q1_DISABLE: u64 = 0x00000620;
    pub const GEM_INT_Q7_DISABLE: u64 = GEM_INT_Q1_DISABLE + 6;

    pub const GEM_INT_Q1_MASK: u64 = 0x00000640;
    pub const GEM_INT_Q7_MASK: u64 = GEM_INT_Q1_MASK + 6;

    pub const GEM_SCREENING_TYPE1_REGISTER_0: u64 = 0x00000500;

    pub const GEM_ST1R_UDP_PORT_MATCH_ENABLE: u64 = 1 << 29;
    pub const GEM_ST1R_DSTC_ENABLE: u64 = 1 << 28;
    pub const GEM_ST1R_UDP_PORT_MATCH_SHIFT: u64 = 12;
    pub const GEM_ST1R_UDP_PORT_MATCH_WIDTH: u64 = 27 - GEM_ST1R_UDP_PORT_MATCH_SHIFT + 1;
    pub const GEM_ST1R_DSTC_MATCH_SHIFT: u64 = 4;
    pub const GEM_ST1R_DSTC_MATCH_WIDTH: u64 = 11 - GEM_ST1R_DSTC_MATCH_SHIFT + 1;
    pub const GEM_ST1R_QUEUE_SHIFT: u64 = 0;
    pub const GEM_ST1R_QUEUE_WIDTH: u64 = 3 - GEM_ST1R_QUEUE_SHIFT + 1;

    pub const GEM_SCREENING_TYPE2_REGISTER_0: u64 = 0x00000540;

    pub const GEM_ST2R_COMPARE_A_ENABLE: u64 = 1 << 18;
    pub const GEM_ST2R_COMPARE_A_SHIFT: u64 = 13;
    pub const GEM_ST2R_COMPARE_WIDTH: u64 = 17 - GEM_ST2R_COMPARE_A_SHIFT + 1;
    pub const GEM_ST2R_ETHERTYPE_ENABLE: u64 = 1 << 12;
    pub const GEM_ST2R_ETHERTYPE_INDEX_SHIFT: u64 = 9;
    pub const GEM_ST2R_ETHERTYPE_INDEX_WIDTH: u64 = 11 - GEM_ST2R_ETHERTYPE_INDEX_SHIFT + 1;
    pub const GEM_ST2R_QUEUE_SHIFT: u64 = 0;
    pub const GEM_ST2R_QUEUE_WIDTH: u64 = 3 - GEM_ST2R_QUEUE_SHIFT + 1;

    pub const GEM_SCREENING_TYPE2_ETHERTYPE_REG_0: u64 = 0x000006e0;
    pub const GEM_TYPE2_COMPARE_0_WORD_0: u64 = 0x00000700;

    pub const GEM_T2CW1_COMPARE_OFFSET_SHIFT: u64 = 7;
    pub const GEM_T2CW1_COMPARE_OFFSET_WIDTH: u64 = 8 - GEM_T2CW1_COMPARE_OFFSET_SHIFT + 1;
    pub const GEM_T2CW1_OFFSET_VALUE_SHIFT: u64 = 0;
    pub const GEM_T2CW1_OFFSET_VALUE_WIDTH: u64 = 6 - GEM_T2CW1_OFFSET_VALUE_SHIFT + 1;

    /*****************************************/
    pub const GEM_NWCTRL_TXSTART: u32 = 0x00000200; /* Transmit Enable */
    pub const GEM_NWCTRL_TXENA: u32 = 0x00000008; /* Transmit Enable */
    pub const GEM_NWCTRL_RXENA: u32 = 0x00000004; /* Receive Enable */
    pub const GEM_NWCTRL_LOCALLOOP: u32 = 0x00000002; /* Local Loopback */

    pub const GEM_NWCFG_STRIP_FCS: u32 = 0x00020000; /* Strip FCS field */
    pub const GEM_NWCFG_LERR_DISC: u32 = 0x00010000; /* Discard RX frames with len err */
    pub const GEM_NWCFG_BUFF_OFST_M: u32 = 0x0000C000; /* Receive buffer offset mask */
    pub const GEM_NWCFG_BUFF_OFST_S: u32 = 14;         /* Receive buffer offset shift */
    pub const GEM_NWCFG_UCAST_HASH: u32 = 0x00000080; /* accept unicast if hash match */
    pub const GEM_NWCFG_MCAST_HASH: u32 = 0x00000040; /* accept multicast if hash match */
    pub const GEM_NWCFG_BCAST_REJ: u32 = 0x00000020; /* Reject broadcast packets */
    pub const GEM_NWCFG_PROMISC: u32 = 0x00000010; /* Accept all packets */

    pub const GEM_DMACFG_ADDR_64B: u32 = 1 << 30;
    pub const GEM_DMACFG_TX_BD_EXT: u32 = 1 << 29;
    pub const GEM_DMACFG_RX_BD_EXT: u32 = 1 << 28;
    pub const GEM_DMACFG_RBUFSZ_M: u32 = 0x00FF0000; /* DMA RX Buffer Size mask */
    pub const GEM_DMACFG_RBUFSZ_S: u32 = 16;         /* DMA RX Buffer Size shift */
    pub const GEM_DMACFG_RBUFSZ_MUL: u32 = 64;         /* DMA RX Buffer Size multiplier */
    pub const GEM_DMACFG_TXCSUM_OFFL: u32 = 0x00000800; /* Transmit checksum offload */

    pub const GEM_TXSTATUS_TXCMPL: u64 = 0x00000020; /* Transmit Complete */
    pub const GEM_TXSTATUS_USED: u64 = 0x00000001; /* sw owned descriptor encountered */

    pub const GEM_RXSTATUS_FRMRCVD: u64 = 0x00000002; /* Frame received */
    pub const GEM_RXSTATUS_NOBUF: u64 = 0x00000001; /* Buffer unavailable */

    /* GEM_ISR GEM_IER GEM_IDR GEM_IMR */
    pub const GEM_INT_TXCMPL: u32 = 0x00000080; /* Transmit Complete */
    pub const GEM_INT_TXUSED: u32 = 0x00000008;
    pub const GEM_INT_RXUSED: u32 = 0x00000004;
    pub const GEM_INT_RXCMPL: u32 = 0x00000002;

    pub const GEM_PHYMNTNC_OP_R: u64 = 0x20000000; /* read operation */
    pub const GEM_PHYMNTNC_OP_W: u64 = 0x10000000; /* write operation */
    pub const GEM_PHYMNTNC_ADDR: u64 = 0x0F800000; /* Address bits */
    pub const GEM_PHYMNTNC_ADDR_SHFT: u64 = 23;
    pub const GEM_PHYMNTNC_REG: u64 = 0x007C0000; /* register bits */
    pub const GEM_PHYMNTNC_REG_SHIFT: u64 = 18;

    /* Marvell PHY definitions */
    pub const BOARD_PHY_ADDRESS: u64 = 23; /* PHY address we will emulate a device at */

    pub const PHY_REG_CONTROL: u64 = 0;
    pub const PHY_REG_STATUS: u64 = 1;
    pub const PHY_REG_PHYID1: u64 = 2;
    pub const PHY_REG_PHYID2: u64 = 3;
    pub const PHY_REG_ANEGADV: u64 = 4;
    pub const PHY_REG_LINKPABIL: u64 = 5;
    pub const PHY_REG_ANEGEXP: u64 = 6;
    pub const PHY_REG_NEXTP: u64 = 7;
    pub const PHY_REG_LINKPNEXTP: u64 = 8;
    pub const PHY_REG_100BTCTRL: u64 = 9;
    pub const PHY_REG_1000BTSTAT: u64 = 10;
    pub const PHY_REG_EXTSTAT: u64 = 15;
    pub const PHY_REG_PHYSPCFC_CTL: u64 = 16;
    pub const PHY_REG_PHYSPCFC_ST: u64 = 17;
    pub const PHY_REG_INT_EN: u64 = 18;
    pub const PHY_REG_INT_ST: u64 = 19;
    pub const PHY_REG_EXT_PHYSPCFC_CTL: u64 = 20;
    pub const PHY_REG_RXERR: u64 = 21;
    pub const PHY_REG_EACD: u64 = 22;
    pub const PHY_REG_LED: u64 = 24;
    pub const PHY_REG_LED_OVRD: u64 = 25;
    pub const PHY_REG_EXT_PHYSPCFC_CTL2: u64 = 26;
    pub const PHY_REG_EXT_PHYSPCFC_ST: u64 = 27;
    pub const PHY_REG_CABLE_DIAG: u64 = 28;

    pub const PHY_REG_CONTROL_RST: u64 = 0x8000;
    pub const PHY_REG_CONTROL_LOOP: u64 = 0x4000;
    pub const PHY_REG_CONTROL_ANEG: u64 = 0x1000;

    pub const PHY_REG_STATUS_LINK: u64 = 0x0004;
    pub const PHY_REG_STATUS_ANEGCMPL: u64 = 0x0020;

    pub const PHY_REG_INT_ST_ANEGCMPL: u64 = 0x0800;
    pub const PHY_REG_INT_ST_LINKC: u64 = 0x0400;
    pub const PHY_REG_INT_ST_ENERGY: u64 = 0x0010;

    /***********************************************************************/
    pub const GEM_RX_REJECT: u64 = (-1i64) as u64;
    pub const GEM_RX_PROMISCUOUS_ACCEPT: u64 = (-2i64) as u64;
    pub const GEM_RX_BROADCAST_ACCEPT: u64 = (-3i64) as u64;
    pub const GEM_RX_MULTICAST_HASH_ACCEPT: u64 = (-4i64) as u64;
    pub const GEM_RX_UNICAST_HASH_ACCEPT: u64 = (-5i64) as u64;

    pub const GEM_RX_SAR_ACCEPT: u64 = 0;

    /***********************************************************************/

    pub const DESC_1_USED: u32 = 0x80000000;
    pub const DESC_1_LENGTH: u32 = 0x00001FFF;

    pub const DESC_1_TX_WRAP: u32 = 0x40000000;
    pub const DESC_1_TX_LAST: u32 = 0x00008000;

    pub const DESC_0_RX_WRAP: u32 = 0x00000002;
    pub const DESC_0_RX_OWNERSHIP: u32 = 0x00000001;

    pub const R_DESC_1_RX_SAR_SHIFT: u32 = 25;
    pub const R_DESC_1_RX_SAR_LENGTH: u32 = 2;
    pub const R_DESC_1_RX_SAR_MATCH: u32 = 1 << 27;
    pub const R_DESC_1_RX_UNICAST_HASH: u32 = 1 << 29;
    pub const R_DESC_1_RX_MULTICAST_HASH: u32 = 1 << 30;
    pub const R_DESC_1_RX_BROADCAST: u32 = 1 << 31;

    pub const DESC_1_RX_SOF: u32 = 0x00004000;
    pub const DESC_1_RX_EOF: u32 = 0x00008000;

    pub const GEM_MODID_VALUE: u64 = 0x00020118;
}
pub use constants::*;

const GEM_DMACFG: u64 = 0x00000010;
const GEM_DMACFG_ADDR_64B: u32 = 1 << 30;


const TX_QUEUE_LENGTH: usize = 1024;
const RX_QUEUE_LENGTH: usize = 128;

const VIRTIO_MTU: u16 = 2048;

#[repr(transparent)]
#[derive(Copy, Clone, Debug)]
struct RxDesc([u32; 4]);

#[repr(transparent)]
#[derive(Copy, Clone, Debug)]
struct TxDesc([u32; 4]);
impl TxDesc {
    fn set_addr(&mut self, addr: u64) {
        self.0[0] = (addr & 0xffffffff) as u32;
        self.0[2] = (addr >> 32) as u32
    }
    fn set_flags(&mut self, flags: u32, size: usize) {
        assert!(size <= DESC_1_LENGTH as usize);
        assert_eq!(flags & DESC_1_LENGTH, 0);
        self.0[1] = (self.0[1] & DESC_1_TX_WRAP) | flags | size as u32;
    }
}

#[repr(align(2048))]
struct RxBuf([u8; 2048]);

/// Driver for the Cadence GEM Ethernet device.
pub struct MacbDriver {
    control_registers: MemoryRegion<u32>,
    mac: [u8; 6],

    rx_queue: [RxDesc; RX_QUEUE_LENGTH],
    tx_queue: [TxDesc; TX_QUEUE_LENGTH],

    rx_buffers: [RxBuf; RX_QUEUE_LENGTH],

    rx_head: usize,
    tx_tail: usize,
}

impl MacbDriver {
    pub const fn new(control_registers: MemoryRegion<u32>) -> Self {
        Self {
            control_registers,
            mac: [0x0E, 0xAD, 0x5D, 0x44, 0xF4, 0x6D],

            rx_queue: [RxDesc([0; 4]); RX_QUEUE_LENGTH],
            tx_queue: [TxDesc([0; 4]); TX_QUEUE_LENGTH],

            rx_buffers: arr![RxBuf([0; 2048]); 128],

            rx_head: 0,
            tx_tail: 0,
        }
    }

    pub fn initialize(&mut self) {
        // let mac_lo = self.control_registers[GEM_SPADDR1LO];
        // let mac_hi = self.control_registers[GEM_SPADDR1HI];

        // self.mac[0] = mac_lo as u8;
        // self.mac[1] = (mac_lo >> 8) as u8;
        // self.mac[2] = (mac_lo >> 16) as u8;
        // self.mac[3] = (mac_lo >> 24) as u8;
        // self.mac[4] = mac_hi as u8;
        // self.mac[5] = (mac_hi >> 8) as u8;

        // for i in 0..6 {
        //     println!("mac[{}] = {:x}", i, self.mac[i]);
        // }

        self.control_registers[GEM_NWCFG] |= GEM_NWCFG_PROMISC;
        // self.control_registers[GEM_SPADDR1LO] = mac_lo;
        // self.control_registers[GEM_SPADDR1HI] = mac_hi;

        // Initialize queue state
        let rx_buffers_ptr = pmap::translate_host_address(self.rx_buffers[0].0.as_ptr() as u64).unwrap().pa;
        for (i, rx) in self.rx_queue.iter_mut().enumerate() {
            let addr = rx_buffers_ptr + 2048 * i as u64;
            rx.0[0] = (addr & 0xffffffff) as u32;
            rx.0[2] = (addr >> 32) as u32
        }
        self.rx_queue[RX_QUEUE_LENGTH - 1].0[1] |= DESC_0_RX_WRAP;

        self.tx_queue[TX_QUEUE_LENGTH - 1].0[0] |= DESC_1_TX_WRAP;
        for tx in self.tx_queue.iter_mut() {
            tx.0[1] = DESC_1_USED;
        }

        // Set control registers
        self.control_registers[GEM_DMACFG] = 0;
        self.control_registers[GEM_DMACFG] |= GEM_DMACFG_ADDR_64B | ((2048 / GEM_DMACFG_RBUFSZ_MUL) << GEM_DMACFG_RBUFSZ_S);

        self.control_registers[GEM_IER] = GEM_INT_RXCMPL;

        let rx_ptr = pmap::translate_host_address(self.rx_queue.as_ptr() as u64).unwrap().pa;
        self.control_registers[GEM_RXQBASE] = rx_ptr as u32;
        self.control_registers[GEM_RBQPH] = (rx_ptr >> 32) as u32;

        let tx_ptr = pmap::translate_host_address(self.tx_queue.as_ptr() as u64).unwrap().pa;
        self.control_registers[GEM_TXQBASE] = tx_ptr as u32;
        self.control_registers[GEM_TBQPH] = (tx_ptr >> 32) as u32;

        self.control_registers[GEM_NWCTRL] = GEM_NWCTRL_RXENA | GEM_NWCTRL_TXENA;
    }

    // Transmit the contents of buffers then return the number of bytes transmitted.
    pub fn transmit(&mut self, buffers: &[&[u8]]) -> Option<u32> {
        // println!("transmit");
        let mut header_bytes_left = 10u64;
        for i in 0..buffers.len() {
            if header_bytes_left >= buffers[i].len() as u64 {
                header_bytes_left -= buffers[i].len() as u64;
                continue;
            }

            let pa = pmap::translate_host_address(buffers[i].as_ptr() as u64).unwrap().pa;
            let desc = &mut self.tx_queue[self.tx_tail];

            while (desc.0[1] & DESC_1_USED) == 0 {
                riscv::barrier();
            }

            if i == buffers.len() - 1 {
                desc.set_addr(pa + header_bytes_left);
                desc.set_flags(DESC_1_TX_LAST, buffers[i].len() - header_bytes_left as usize);
            } else {
                desc.set_addr(pa + header_bytes_left);
                desc.set_flags(0, buffers[i].len() - header_bytes_left as usize);
            };

            self.tx_tail = (self.tx_tail + 1) % self.tx_queue.len();
        }

        self.control_registers[GEM_NWCTRL] = GEM_NWCTRL_TXSTART | GEM_NWCTRL_RXENA | GEM_NWCTRL_TXENA;
        self.control_registers[GEM_NWCTRL] = GEM_NWCTRL_RXENA | GEM_NWCTRL_TXENA;

        Some(buffers.iter().map(|b|b.len()).sum::<usize>() as u32)
    }

    pub fn receive(&mut self, local: &mut LocalContext, guest_memory: &mut MemoryRegion) {
        // println!("receive");

        while self.rx_queue[self.rx_head].0[1] & DESC_1_RX_SOF != 0 {
            assert!(self.rx_queue[self.rx_head].0[1] & DESC_1_RX_EOF != 0); // All packets must fit in one buffer

            let len = (self.rx_queue[self.rx_head].0[1] & DESC_1_LENGTH) as u64;
            let slice = &self.rx_buffers[self.rx_head].0[..(len as usize)];

            let mut dropped = true;

            local.with_ranges(guest_memory, VIRTIO_NET_RX_QUEUE, |guest_memory, ranges|{
                if !dropped { return None; }
                dropped = false;

                assert!(ranges.len() >= 2);
                assert_eq!(ranges[0].end - ranges[0].start, 10);
                assert!(ranges[1].end - ranges[1].start > len);
                guest_memory.slice_mut(ranges[1].start, len).copy_from_slice(slice);
                Some(len as u32 + 10)
            });

            if dropped {
                for (i, b) in slice.iter().enumerate() {
                    if i != 0 && i % 16 == 0 {
                        println!("");
                    } else if i % 16 == 8 {
                        print!("  ");
                    }
                    print!("{:02x} ", b);
                }
                println!("\n");
            }

            self.rx_queue[self.rx_head].0[1] &= DESC_0_RX_WRAP;
            self.rx_head = (self.rx_head + 1) % RX_QUEUE_LENGTH;
            println!("rx_head = {}", self.rx_head);
        }

        // local.with_ranges(guest_memory, VIRTIO_NET_RX_QUEUE, |guest_memory, ranges|{
        //     if self.rx_queue[self.rx_head].0[1] & DESC_1_RX_SOF != 0 {
        //         assert_eq!(ranges[0].end - ranges[0].start, 10);
        //         assert!(ranges.len() >= 2);
        //         assert!(self.rx_queue[self.rx_head].0[1] & DESC_1_RX_EOF != 0); // All packets must fit in one buffer

        //         let len = (self.rx_queue[self.rx_head].0[1] & DESC_1_LENGTH) as u64;
        //         assert!(ranges[1].end - ranges[1].start > len);

        //         guest_memory.slice_mut(ranges[1].start, len)
        //             .copy_from_slice(&self.rx_buffers[self.rx_head].0[..(len as usize)]);


        //         // for (i, b) in guest_memory.slice_mut(ranges[1].start, len-4).iter().enumerate() {
        //         //     if i != 0 && i % 16 == 0 {
        //         //         println!("");
        //         //     } else if i % 16 == 8 {
        //         //         print!("  ");
        //         //     }
        //         //     print!("{:02x} ", b);
        //         // }
        //         // println!("\n");

        //         self.rx_queue[self.rx_head].0[1] = 0;
        //         self.rx_head = (self.rx_head + 1) % RX_QUEUE_LENGTH;
        //         // TODO: Free descriptor

        //         Some(len as u32 + 10)
        //     } else {
        //         None
        //     }
        // });
    }
}

impl Driver for MacbDriver {
    const DEVICE_ID: u32 = 1;
    const FEATURES: u64 = VIRTIO_NET_F_MAC | VIRTIO_NET_F_MTU;
    const QUEUE_NUM_MAX: u32 = 32;
    const MAX_CONTEXTS: u64 = 1;

    fn demux_interrupt(&mut self) -> u64 {
        println!("MACB: Demux interrupt!");
        0
    }
    fn interrupt(&mut self, local: &mut LocalContext, guest_memory: &mut MemoryRegion) -> bool {
        if !local.driver_ok() {
            println!("driver not ok");
            return false;
        }

        self.receive(local, guest_memory);
        if local.interrupt_status == 0 {
            println!("ignoring interrupt");
        }
        local.interrupt_status != 0
    }
    fn doorbell(&mut self, local: &mut LocalContext, guest_memory: &mut MemoryRegion, queue: u32) {
        // println!("doorbell (queue = {}):\n---------------------", queue);
        // // local.get_queue(guest_memory, 0);
        // // local.get_queue(guest_memory, 1);
        // for i in 0..1 {
        //     println!("  queue[{}].pfn = {:x}", i, local.queue_pfn[i as usize]);
        //     println!("  queue[{}].num = {:x}", i, local.queue_num[i as usize]);
        //     println!("  queue[{}].align = {:x}", i, local.queue_align[i as usize]);
        //     println!("  queue[{}].avail_idx = {}", i, local.get_queue(guest_memory, i).avail_idx());
        //     println!("  queue[{}].used_idx = {}", i, local.get_queue(guest_memory, i).used_idx());
        // }
        match queue {
            VIRTIO_NET_TX_QUEUE => local.with_buffers(guest_memory, queue, |b| self.transmit(b)),
            VIRTIO_NET_RX_QUEUE => {
                self.receive(local, guest_memory);
                // local.get_queue(guest_memory, queue).set_used_flags(VIRTQ_USED_F_NO_NOTIFY)
            }
            _ => {}
        }
    }

    fn read_config_u8(&mut self, _local: &mut LocalContext, offset: u64) -> u8 {
        match offset {
            0..=5 => self.mac[offset as usize],
            10 => VIRTIO_MTU.to_le_bytes()[0],
            11 => VIRTIO_MTU.to_le_bytes()[1],
            _ => 0
        }
    }
    fn write_config_u8(&mut self, _local: &mut LocalContext, offset: u64, value: u8) {
        match offset {
            0..=5 => {
                self.mac[offset as usize] = value;
                unimplemented!(); // TODO: set device MAC to updated value
            }
            _ => {}
        }
    }

    fn reset_context(&mut self, local: &mut LocalContext) {
        local.reset();
    }
}
