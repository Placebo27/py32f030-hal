//! # ADC 主要特性
//! ## 高性能
//! - 12bit、10bit、8bit 和 6bit 分辨率可配置
//! - ADC 转换时间：1us@12bit（1MHz）
//! - 自校准
//! - 可编程的采样时间
//! - 可编程的数据对齐模式
//! - 支持 DMA
//! ## 低功耗
//! - 为低功耗操作，降低 PCLK 频率，而仍然维持合适的 ADC 性能
//! - 等待模式：防止以低频 PCLK 运行产生溢出
//! ## 模拟输入通道
//! - 10 个外部模拟输入通道：PA[7:0]和 PB[1:0]
//! - 1 个内部 temperature sensor 通道
//! - 1 个内部参考电压通道（VREFINT）
//! ## 转换操作启动可以通过
//! - 软件启动
//! - 可配置极性的硬件启动（TIM1、TIM3 或者 GPIO）
//! ## 转换模式
//! - 单次模式(single mode)：可以转换 1 个单通道或者可以扫描一系列通道
//! - 连续模式(continuous mode)：连续转换被选择的通道
//! - 不连续模式(discontinuous mode)：每次触发，转换被选择的通道 1 次
//! ## 中断产生
//! - 在采样结束
//! - 在转换结束
//! - 在连续转换结束
//! - 模拟看门狗事件
//! - 溢出事件
//! ## 模拟看门狗

mod hal;
mod pins;

use core::{future::Future, marker::PhantomData, task::Poll};

use crate::{
    clock::peripheral::PeripheralInterrupt,
    macro_def::impl_sealed_peripheral_id,
    mcu::peripherals::ADC,
    mode::{Async, Blocking},
};

use embassy_hal_internal::Peripheral;
pub use pins::{TemperatureChannel, VRrefChannel};

use crate::{
    clock::peripheral::{PeripheralClockIndex, PeripheralEnable},
    delay::wait_for_true_timeout_block,
    mode::Mode,
};

use embassy_sync::waitqueue::AtomicWaker;

static ADC_INT_WAKER: [AtomicWaker; 1] = [AtomicWaker::new()];

#[allow(private_bounds)]
pub trait Instance: Peripheral<P = Self> + hal::sealed::Instance + 'static + Send {}

#[derive(PartialEq)]
pub(crate) enum Id {
    ADC1 = 0,
}

impl_sealed_peripheral_id!(ADC, ADC1);

impl PeripheralEnable for Id {
    fn clock(&self, en: bool) {
        match *self {
            Self::ADC1 => PeripheralClockIndex::ADC.clock(en),
        }
    }

    fn is_open(&self) -> bool {
        match *self {
            Self::ADC1 => PeripheralClockIndex::ADC.is_open(),
        }
    }

    fn reset(&self) {
        match *self {
            Self::ADC1 => PeripheralClockIndex::ADC.reset(),
        }
    }
}

impl PeripheralInterrupt for Id {
    fn interrupt(&self) -> crate::pac::interrupt {
        match *self {
            Self::ADC1 => crate::pac::interrupt::ADC_COMP,
        }
    }
}

/// ADC clock mode, software can set and clear this bit to define the clock source of the analog ADC
#[derive(Debug)]
pub enum ClockMode {
    PCLK = 0,
    PLCK_DIV2 = 1,
    PCLK_DIV4 = 2,
    PCLK_DIV8 = 3,
    PCLK_DIV16 = 4,
    PCLK_DIV32 = 5,
    PCLK_DIV64 = 6,
    HSI = 0b1000,
    HSI_DIV2 = 0b1001,
    HSI_DIV4 = 0b1010,
    HSI_DIV8 = 0b1011,
    HSI_DIV16 = 0b1100,
    HSI_DIV32 = 0b1101,
    HSI_DIV64 = 0b1110,
}

#[derive(Clone, Copy, PartialEq)]
pub enum AdcChannel {
    /// PA0
    Channel0 = 0,
    /// PA1
    Channel1 = 1,
    /// PA2
    Channel2 = 2,
    /// PA3
    Channel3 = 3,
    /// PA4
    Channel4 = 4,
    /// PA5
    Channel5 = 5,
    /// PA6
    Channel6 = 6,
    /// PA7
    Channel7 = 7,
    /// PB0
    Channel8 = 8,
    /// PB1
    Channel9 = 9,

    ///  nner temperature
    Channel11 = 11,
    /// inner ref voltage
    Channel12 = 12,
}

/// discontinuous mode and Single/Continuous Conversion Mode
#[derive(PartialEq)]
pub enum ConversionMode {
    /// 单次转换模式 (CONT=0, DISCEN=0)
    Single,
    /// 连续转换模式 (CONT=1)
    Continuous,
    /// 非连续转换模式 (DISCEN=1)
    Discontinuous,
}

/// External trigger enable and polarity selection
#[derive(PartialEq)]
pub enum TrigleSignal {
    Soft,
    Rising(ExitTrigleSource),
    Falling(ExitTrigleSource),
    RisingFalling(ExitTrigleSource),
}

/// External trigger selection
#[derive(PartialEq)]
pub enum ExitTrigleSource {
    TIM1_TRG0 = 0,
    TIM1_CC4 = 1,
    TIM3_TRGP = 3,
}

/// Data alignment
#[derive(PartialEq)]
pub enum Align {
    Right,
    Left,
}

/// Data resolution
/// Software sets this bit to select the conversion resolution
pub enum Resolution {
    Bit12 = 0,
    Bit10 = 1,
    Bit8 = 2,
    Bit6 = 3,
}

/// Scan sequence direction
/// Software can set and clear this bit to select the scan sequence direction
#[derive(PartialEq)]
pub enum ScanDir {
    Up,
    Down,
}

/// DMA Configuration
/// This bit can be set and cleared by software, selects between two DMA modes of operation and is valid when DMAEN = 1
#[derive(PartialEq)]
pub enum DmaMode {
    Single,
    Cycle,
}

/// Sampling time selection
/// Software configurable bit selects the sampling time for all channels
pub enum SampleCycles {
    Cycle_3_5 = 0,
    Cycle_5_5 = 1,
    Cycle_7_5 = 2,
    Cycle_13_5 = 3,
    Cycle_28_5 = 4,
    Cycle_41_5 = 5,
    Cycle_71_5 = 6,
    Cycle_239_5 = 7,
}

#[derive(Clone, Copy, PartialEq)]
pub enum CalibrationSampleTime {
    Cycle_1 = 3,
    Cycle_2 = 0,
    Cycle_4 = 1,
    Cycle_8 = 2,
}

#[derive(Clone, Copy, PartialEq)]
pub enum CalibrationSelect {
    Offset = 0,
    OffsetLinearity = 1,
}

#[derive(Debug)]
pub enum Error {
    Busy,
    Timeout,
    Calibrate,
}

pub struct AnyAdc<'d, T: Instance, M: Mode> {
    t: PhantomData<&'d T>,
    _m: PhantomData<M>,
}

impl<'d, T: Instance, M: Mode> AnyAdc<'d, T, M> {
    pub fn new(
        _adc: impl Peripheral<P = T>,
        config: Config,
        channel_config: ChannelConfig,
        channels: &[AdcChannel],
    ) -> Result<Self, Error> {
        T::id().open();
        // T::reset();

        Self::new_inner(config, channel_config, channels)?;

        T::enable();

        Ok(Self {
            t: PhantomData,
            _m: PhantomData,
        })
    }

    /// 校准 adc
    fn calibration(config: CalibrationConfig, timeout: usize) -> Result<(), Error> {
        T::set_calibration_content(config.content);
        T::set_calibration_sample_time(config.sample_time);
        T::calibration_start();

        let block = T::block();
        wait_for_true_timeout_block(timeout, || {
            block.ccsr.read().calon().bit_is_clear() && block.ccsr.read().calfail().bit_is_clear()
        })
        .map_err(|_| Error::Calibrate)?;

        Ok(())
    }

    #[inline]
    pub fn start(&self) {
        T::start();
    }

    #[inline]
    pub fn stop(&self) {
        T::stop()
    }

    fn new_inner(
        config: Config,
        channel_config: ChannelConfig,
        channels: &[AdcChannel],
    ) -> Result<(), Error> {
        const CALIBRATE_TIMEOUT: usize = 1000000;
        T::disable();
        // 设置时钟
        T::set_clock_mode(config.clock);
        T::set_resolution(config.resolution);
        T::set_sample_cycle(config.sample_cycle);
        // 上点后硬件会自动校准一次
        if config.calibration {
            // 必须先校准再开启时钟
            Self::calibration(Default::default(), CALIBRATE_TIMEOUT)?
        }
        T::align(config.align);

        Self::channel_config(channel_config);

        // 使能通道
        for channel in channels {
            T::channel_enable(*channel, true)
        }
        Ok(())
    }

    pub fn channel_enable(&self, channels: &[impl AnalogPin<T>]) {
        for channel in channels {
            channel.as_anlog();
            T::channel_enable(channel.channel(), true);
        }
    }

    pub fn set_watchdog(config: Option<WatchDogConfig>) {
        if let Some(config) = config {
            T::set_watch_dog_threshold(config.high, config.low)
        }
    }

    fn channel_config(config: ChannelConfig) {
        T::conversion_mode(config.mode);
        T::set_scan_dir(config.scan_dir);
        T::set_overwrite(config.over_write);
        T::trigle_signal(config.signal);
    }

    fn is_eoc() -> bool {
        T::is_eoc()
    }

    #[inline]
    fn on_interrupt() {
        // 关闭中断
        ADC_INT_WAKER[T::id() as usize].wake()
    }

    pub fn enable_interrupt(en: bool) {
        unsafe {
            if en {
                cortex_m::peripheral::NVIC::unmask(interrupt::ADC_COMP)
            } else {
                cortex_m::peripheral::NVIC::mask(interrupt::ADC_COMP)
            }
        }
    }
}

pub struct ChannelInputFuture<T: Instance> {
    channel: AdcChannel,
    _t: PhantomData<T>,
}

impl<T: Instance> ChannelInputFuture<T> {
    /// 新建一个 eoc 中断
    /// 记得提前打开 ADC 的总中断，改任务会暂停在 异步中
    pub fn new(channel: AdcChannel) -> Self {
        // 开启通道
        T::channel_enable_exclusive(channel);
        T::clear_eoc();
        T::enable_eoc_interrupt(true);

        // 软件触发，则先触发一次
        if T::is_soft_trigle() {
            T::start();
        }

        Self {
            channel,
            _t: PhantomData,
        }
    }
}

impl<T: Instance> Future for ChannelInputFuture<T> {
    type Output = u16;
    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        ADC_INT_WAKER[T::id() as usize].register(cx.waker());

        if T::is_eoc() {
            // 读取 dr 寄存器会自动清除 eoc 位
            Poll::Ready(T::data_read())
        } else {
            Poll::Pending
        }
    }
}

impl<T: Instance> Drop for ChannelInputFuture<T> {
    fn drop(&mut self) {
        // 关闭中断
        T::enable_eoc_interrupt(false);
    }
}

// impl<T: Instance> Unpin for ChannelInput<T> {}

impl<'d, T: Instance> AnyAdc<'d, T, Blocking> {
    pub fn read_block(&self, timeout: usize) -> Result<u16, Error> {
        // 软件触发，则先触发一次
        if T::is_soft_trigle() {
            T::start();
        }
        wait_for_true_timeout_block(timeout, || T::is_eoc()).map_err(|_| Error::Timeout)?;
        Ok(T::data_read())
    }
}

impl<'d, T: Instance> AnyAdc<'d, T, Async> {
    pub async fn read(&self, channel: impl AnalogPin<T>) -> u16 {
        ChannelInputFuture::<T>::new(channel.channel()).await
    }
}

#[derive(Clone, Copy)]
struct CalibrationConfig {
    content: CalibrationSelect,
    sample_time: CalibrationSampleTime,
}

impl Default for CalibrationConfig {
    fn default() -> Self {
        Self {
            content: CalibrationSelect::OffsetLinearity,
            sample_time: CalibrationSampleTime::Cycle_8,
        }
    }
}

pub struct WatchDogConfig {
    high: u16,
    low: u16,
    // interrupt: bool,
}

pub struct Config {
    /// 是否初始化前是否开始校验
    calibration: bool,
    /// 采样周期
    sample_cycle: SampleCycles,
    /// adc 精度
    resolution: Resolution,
    /// 数据对齐
    align: Align,
    ///  adc 时钟源
    clock: ClockMode,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            calibration: true,
            sample_cycle: SampleCycles::Cycle_7_5,
            resolution: Resolution::Bit12,
            align: Align::Right,
            clock: ClockMode::PCLK,
        }
    }
}

pub struct ChannelConfig {
    /* 转换模式 */
    mode: ConversionMode,
    scan_dir: ScanDir,
    over_write: bool,
    /// 触发信号类型
    signal: TrigleSignal,
}

impl ChannelConfig {
    pub fn mode(self, mode: ConversionMode) -> Self {
        Self { mode, ..self }
    }

    pub fn scan_dir(self, scan_dir: ScanDir) -> Self {
        Self { scan_dir, ..self }
    }

    pub fn over_write(self, over_write: bool) -> Self {
        Self { over_write, ..self }
    }
    /// 多通道配置读取推荐配置
    /// 连续转换/向上扫描/不过写/软件触发
    pub fn new_multiple_channel_perferred() -> Self {
        Self {
            mode: ConversionMode::Continuous,
            scan_dir: ScanDir::Up,
            over_write: false,
            signal: TrigleSignal::Soft,
        }
    }

    /// 单通道读取推荐配置
    /// 连续转换/向上扫描/过写/软件触发
    pub fn new_exclusive_perferred() -> Self {
        Self {
            mode: ConversionMode::Continuous,
            scan_dir: ScanDir::Up,
            over_write: true,
            signal: TrigleSignal::Soft,
        }
    }

    pub fn new_exclusive_single() -> Self {
        Self {
            mode: ConversionMode::Single,
            scan_dir: ScanDir::Up,
            over_write: true,
            signal: TrigleSignal::Soft,
        }
    }
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            mode: ConversionMode::Continuous,
            scan_dir: ScanDir::Up,
            over_write: true,
            signal: TrigleSignal::Soft,
        }
    }
}

pub trait AnalogPin<T: Instance> {
    fn channel(&self) -> AdcChannel;
    fn as_anlog(&self);
}

pub fn temperature(dr: u16) -> f32 {
    const TS_CAL1_ADDR: u32 = 0x1fff_0f14;
    const TS_CAL2_ADDR: u32 = 0x1fff_0f18;

    let ts_cal2 = unsafe { core::ptr::read(TS_CAL2_ADDR as *const u32) } as f32;
    let ts_cal1 = unsafe { core::ptr::read(TS_CAL1_ADDR as *const u32) } as f32;

    // dr as f32 / 4095.0 * 3.3
    (((85.0 - 30.0) / (ts_cal2 - ts_cal1)) * (dr as f32 - ts_cal1)) + 30.0
}

pub fn vrefence_internal(dr: u16) -> f32 {
    // dr as f32 / 4095.0 * 3.3
    4095.0 * 1.2 / dr as f32
}

use crate::pac::interrupt;
#[interrupt]
fn ADC_COMP() {
    // ADC1 的中断 eoc
    if AnyAdc::<ADC, Blocking>::is_eoc() {
        AnyAdc::<ADC, Blocking>::on_interrupt()
    }
    // TODO!
    // comp 的中断
}
