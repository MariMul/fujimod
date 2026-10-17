import * as native from '../pkg/fujimod.js';
export { default as initialize } from '../pkg/fujimod.js';
export type { Session, Device, InfoReply, ConfigurationGeometry, DiagnosticReply, ConsoleReply, LogReply, CapabilitiesReply, FirmwareDownloadReply, DataWindow, UploadReply, ResearchRequest, ResearchReply, App80Request, Mailbox } from '../pkg/fujimod.js';
export type { FirmwareIdentity, ServiceModes, ServiceDefinition, ServiceEntry, ServiceMailbox,
  ModeAvailability, ModeTransition, DefinitionFailure, CommandFailure, App80Command,
  ProfileAction, App80Invocation, App80CommandReply, App80ReplyValue, SaveStatus,
  WordReadResult, WordWriteResult, RegisterAction, I2cLength, I2cPayload, I2cAction, DisplayOptionAction, SoundAction, HdmiAction, SoundClip, HdmiValue, HdmiPacket, ChargeAction, ChargeResult, ChargePresence, ReportedChargeState, AudioCount, AudioResult, StorageEraseResult, StorageStatus, RadioAction, RadioChannel, RadioPower, PeerAddress, NandReadResult, Option41State } from '../pkg/fujimod.js';

class ChunkIterator implements AsyncIterableIterator<Uint8Array> {
  constructor(private readonly reader: native.RangeReader) {}
  get bytesCompleted(): number { return this.reader.bytesCompleted; }
  close(): void { this.reader.close(); }
  cancelPending(): void { this.reader.cancelPending(); }
  async next(): Promise<IteratorResult<Uint8Array>> {
    const value = await this.reader.nextChunk();
    return value === undefined ? { done: true, value: undefined } : { done: false, value };
  }
  async return(): Promise<IteratorResult<Uint8Array>> {
    this.close();
    return { done: true, value: undefined };
  }
  [Symbol.asyncIterator](): AsyncIterableIterator<Uint8Array> { return this; }
}

/** One cohesive namespace; protocol operations execute in the Rust core. */
export interface UsbError extends Error {
  readonly kind: native.ErrorKind;
  readonly stage?: native.TraceStage;
  readonly requestKind?: native.TraceRequestKind;
  readonly transferKind?: native.TransferFailureKind;
  readonly protocolKind?: native.ProtocolFailureKind;
  readonly command?: number; readonly primary?: number; readonly secondary?: number;
  readonly transferred?: number; readonly expected?: number; readonly actual?: number;
  readonly available?: number; readonly declared?: number; readonly requested?: number; readonly limit?: number;
  readonly reserved?: number; readonly backendCode?: number;
  readonly partial?: Uint8Array; readonly cause?: UsbError;
  readonly reason?: native.DefinitionFailure | native.CommandFailure;
}
function isUsbError(error: unknown): error is UsbError {
  return error instanceof Error && 'kind' in error && typeof error.kind === 'number' && Object.values(native.ErrorKind).includes(error.kind);
}
export interface DefinitionError extends UsbError {
  readonly kind: native.ErrorKind.Definition;
  readonly reason: native.DefinitionFailure;
}
export interface ServiceCommandError extends UsbError {
  readonly kind: native.ErrorKind.ServiceCommand;
  readonly reason: native.CommandFailure;
}
function isDefinitionError(error: unknown): error is DefinitionError {
  return isUsbError(error) && error.kind === native.ErrorKind.Definition;
}
function isServiceCommandError(error: unknown): error is ServiceCommandError {
  return isUsbError(error) && error.kind === native.ErrorKind.ServiceCommand;
}
/** Caller-selected data shared by USB and future emulator/SDK consumers. */
export const defs = Object.freeze({
  FirmwareIdentity: native.FirmwareIdentity,
  DefinitionFailureKind: native.DefinitionFailureKind,
  isError: isDefinitionError,
  service: Object.freeze({
    Definition: Object.freeze({ fromJson: native.ServiceDefinition.fromJson }),
    ServiceModes: native.ServiceModes,
    Codec: native.ServiceCodec,
    ModeAvailabilityKind: native.ModeAvailabilityKind,
    ModeTransitionKind: native.ModeTransitionKind,
  }),
});
export const usb = Object.freeze({
  Device: native.Device,
  Session: native.Session,
  SessionState: native.SessionState,
  ErrorKind: native.ErrorKind,
  ProtocolFailureKind: native.ProtocolFailureKind,
  TransferFailureKind: native.TransferFailureKind,
  TransferStage: native.TraceStage,
  RequestKind: native.TraceRequestKind,
  isError: isUsbError,
  DeviceFilter: native.DeviceFilter,
  discover: native.discover,
  requestDevice: native.requestDevice,
  FirmwareLimits: native.FirmwareLimits,
  InterfaceSelection: native.InterfaceSelection,
  TransferOptions: native.TransferOptions,
  InfoSelector: native.InfoSelector,
  InfoReply: native.InfoReply,
  ConfigurationView: native.ConfigurationView,
  ConfigurationGeometry: native.ConfigurationGeometry,
  DiagnosticTag: native.DiagnosticTag,
  DiagnosticReply: native.DiagnosticReply,
  CommandStatus: native.CommandStatus,
  KeyState: native.KeyState,
  CameraMode: native.CameraMode,
  DataReadMode: native.DataReadMode,
  DataWriteMode: native.DataWriteMode,
  ConsoleProgress: native.ConsoleProgress,
  ConsoleReply: native.ConsoleReply,
  LogReply: native.LogReply,
  CapabilitiesReply: native.CapabilitiesReply,
  FirmwareDownloadState: native.FirmwareDownloadState,
  FirmwareDownloadReply: native.FirmwareDownloadReply,
  DataWindow: native.DataWindow,
  OffsetMode: native.OffsetMode,
  memory: Object.freeze({
    readRange: native.readMemoryRange,
    writeRange: native.writeMemoryRange,
    readChunks(session: native.Session, address: number, length: number): ChunkIterator {
      return new ChunkIterator(native.memoryChunks(session, address, length));
    },
  }),
  data: Object.freeze({
    upload: native.uploadData,
    readChunks(session: native.Session, window: native.DataWindow, address: number, length: number): ChunkIterator {
      return new ChunkIterator(native.dataChunks(session, window, address, length));
    },
  }),
  app80: Object.freeze({
    CommandCode: native.App80CommandCode, Request: native.App80Request,
    Mailbox: native.Mailbox, exchange: native.exchangeApp80,
    Service: native.App80Service,
    Command: Object.freeze({
      writePins: native.App80Command.writePins, readPins: native.App80Command.readPins,
      readChannel: native.App80Command.readChannel, writeChannel: native.App80Command.writeChannel,
      profileByte: native.App80Command.profileByte, downloadStatus: native.App80Command.downloadStatus,
      copyConfiguration: native.App80Command.copyConfiguration, flushConfiguration: native.App80Command.flushConfiguration,
      readWord: native.App80Command.readWord, writeWord: native.App80Command.writeWord,
      registerControl: native.App80Command.registerControl, i2c: native.App80Command.i2c,
      accumulator: native.App80Command.accumulator, peripheralReport: native.App80Command.peripheralReport,
      displayOption: native.App80Command.displayOption, displayMode: native.App80Command.displayMode,
      sound: native.App80Command.sound, hdmi: native.App80Command.hdmi, parameterService: native.App80Command.parameterService,
      chargeControl: native.App80Command.chargeControl, chargeStatus: native.App80Command.chargeStatus,
      nandReadBlocks: native.App80Command.nandReadBlocks, option41: native.App80Command.option41,
      wlanControl: native.App80Command.wlanControl, radioControl: native.App80Command.radioControl,
      signalStrength: native.App80Command.signalStrength, iperf: native.App80Command.iperf,
      audioCapture: native.App80Command.audioCapture, audioPlayback: native.App80Command.audioPlayback,
      eraseStorage: native.App80Command.eraseStorage, storageStatus: native.App80Command.storageStatus,
      formatStorage: native.App80Command.formatStorage, displaySetup: native.App80Command.displaySetup,
    }),
    ProfileAction: Object.freeze({ read: native.ProfileAction.read, write: native.ProfileAction.write, reset: native.ProfileAction.reset }),
    ProfileBank: native.ProfileBank,
    RegisterAction: Object.freeze({ audioValue: native.RegisterAction.audioValue,
      displayFirst: native.RegisterAction.displayFirst, displayStub: native.RegisterAction.displayStub,
      displaySecond: native.RegisterAction.displaySecond, powerWrite: native.RegisterAction.powerWrite,
      positionPair: native.RegisterAction.positionPair, noopFirst: native.RegisterAction.noopFirst, noopSecond: native.RegisterAction.noopSecond }),
    I2cLength: native.I2cLength, I2cPayload: native.I2cPayload, I2cPort: native.I2cPort,
    I2cAction: Object.freeze({ read: native.I2cAction.read, write: native.I2cAction.write, writeSingle: native.I2cAction.writeSingle }),
    AccumulatorAction: native.AccumulatorAction,
    Toggle: native.Toggle, DisplayMode: native.DisplayMode,
    DisplayOptionAction: Object.freeze({idlePins: native.DisplayOptionAction.idlePins, forwardPair: native.DisplayOptionAction.forwardPair}),
    SoundClip: native.SoundClip, SoundFormat: native.SoundFormat, SoundTrack: native.SoundTrack, SoundOperation: native.SoundOperation,
    SoundAction: Object.freeze({clip: native.SoundAction.clip, track: native.SoundAction.track, noop: native.SoundAction.noop}),
    HdmiValue: native.HdmiValue, HdmiPacket: native.HdmiPacket,
    HdmiAction: Object.freeze({resetStub: native.HdmiAction.resetStub, packet: native.HdmiAction.packet, triplet: native.HdmiAction.triplet}),
    ChargeAction: Object.freeze({control: native.ChargeAction.control, enableAndSetFlag: native.ChargeAction.enableAndSetFlag}),
    NandSpare: native.NandSpare, NandReadResultKind: native.NandReadResultKind,
    Option41StateKind: native.Option41StateKind, PeerAddress: native.PeerAddress,
    RadioMode: native.RadioMode, RadioChannel: native.RadioChannel, RadioPower: native.RadioPower,
    RadioAction: Object.freeze({primary:native.RadioAction.primary,secondary:native.RadioAction.secondary}),
    AudioCount: native.AudioCount, AudioRoute: native.AudioRoute, AudioPlayback: native.AudioPlayback,
    StorageErase: native.StorageErase, AudioResultKind: native.AudioResultKind,
    StorageEraseResultKind: native.StorageEraseResultKind, StorageStatusKind: native.StorageStatusKind,
    ChargeQuery: native.ChargeQuery, ChargeResultKind: native.ChargeResultKind,
    ChargePresenceKind: native.ChargePresenceKind, ReportedChargeStateKind: native.ReportedChargeStateKind,
    PinBank: native.PinBank, ProfileIndex: native.ProfileIndex, WordAddress: native.WordAddress,
    ReplyKind: native.App80ReplyKind, SaveStatusKind: native.SaveStatusKind,
    WordReadKind: native.WordReadKind, WordWriteKind: native.WordWriteKind,
    CommandFailureKind: native.CommandFailureKind, ArgumentKind: native.ArgumentKind,
    isError: isServiceCommandError, execute: native.executeApp80,
  }),
  research: Object.freeze({ CommandCode: native.ResearchCommandCode, Request: native.ResearchRequest, Reply: native.ResearchReply }),
  tracing: Object.freeze({ Trace: native.Trace, Stage: native.TraceStage, Phase: native.TracePhase, RequestKind: native.TraceRequestKind, FailureKind: native.TraceFailureKind }),
});

/** Make this initialized Rust module available to the FujiMod Python package. */
export function registerPyodide(runtime: { registerJsModule(name: string, module: object): void }): void {
  runtime.registerJsModule('_fujimod_js', { usb, defs });
}
