import { WasiCliExit } from './interfaces/wasi-cli-exit';
import { WasiCliStderr } from './interfaces/wasi-cli-stderr';
import { WasiCliStdin } from './interfaces/wasi-cli-stdin';
import { WasiCliStdout } from './interfaces/wasi-cli-stdout';
import { WasiCliTerminalInput } from './interfaces/wasi-cli-terminal-input';
import { WasiCliTerminalOutput } from './interfaces/wasi-cli-terminal-output';
import { WasiCliTerminalStderr } from './interfaces/wasi-cli-terminal-stderr';
import { WasiCliTerminalStdin } from './interfaces/wasi-cli-terminal-stdin';
import { WasiCliTerminalStdout } from './interfaces/wasi-cli-terminal-stdout';
import { WasiFilesystemPreopens } from './interfaces/wasi-filesystem-preopens';
import { WasiFilesystemTypes } from './interfaces/wasi-filesystem-types';
import { WasiIoStreams } from './interfaces/wasi-io-streams';
export function concat(left: string, right: string): string;
export function add(left: number, right: number): number;
