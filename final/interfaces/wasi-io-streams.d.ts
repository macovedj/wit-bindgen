export namespace WasiIoStreams {
  export function read(this_: InputStream, len: bigint): [Uint8Array, StreamStatus];
  export function blockingRead(this_: InputStream, len: bigint): [Uint8Array, StreamStatus];
  export function dropInputStream(this_: InputStream): void;
  export function checkWrite(this_: OutputStream): bigint;
  export function write(this_: OutputStream, contents: Uint8Array): void;
  export function blockingWriteAndFlush(this_: OutputStream, contents: Uint8Array): void;
  export function blockingFlush(this_: OutputStream): void;
  export function dropOutputStream(this_: OutputStream): void;
}
export type InputStream = number;
/**
 * # Variants
 * 
 * ## `"open"`
 * 
 * ## `"ended"`
 */
export type StreamStatus = 'open' | 'ended';
export type OutputStream = number;
/**
 * # Variants
 * 
 * ## `"last-operation-failed"`
 * 
 * ## `"closed"`
 */
export type WriteError = 'last-operation-failed' | 'closed';
