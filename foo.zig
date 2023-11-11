const std = @import("std");
const mem = std.mem;
var gpa = std.heap.GeneralPurposeAllocator(.{}){};
const allocator = gpa.allocator();

fn alloc(len: usize) [*]u8 {
  const buf = allocator.alloc(u8, len) catch |e| {
    std.debug.panic("FAILED TO ALLOC MEM {}", .{e});
  };
  return buf.ptr;
}

export fn cabi_realloc(origPtr: *[]u8, origSize: u8, alignment: u8, newSize: u8) ?[*]u8 {
  _ = origSize;
  _ = alignment;
  const buf = allocator.realloc(origPtr.*, newSize) catch {
    return null;
  };
  return buf.ptr;
}

export fn __export_concat(leftPtr: [*]u8, leftLength: u32, rightPtr: [*]u8, rightLength: u32, ) [*]u8{
  const left = leftPtr[0..leftLength];
  const right = rightPtr[0..rightLength];
  const result = Guest.concat(left, right);
  const ret = alloc(8);
  std.mem.writeIntLittle(u32, ret[0..4], @intCast(@intFromPtr(result.ptr)));
  std.mem.writeIntLittle(u32, ret[4..8], @intCast(result.len));
  return ret;
}

export fn __post_return_concat(arg: u32) void {
  var buffer: [8]u8 = .{0} ** 8;
  std.mem.writeIntNative(u32, buffer[0..][0..@sizeOf(u32)], arg);
  const stringPtr = buffer[0..4];
  const stringSize = buffer[4..8];
  const bytesPtr = std.mem.readIntLittle(u32, @ptrCast(stringPtr));
  const ptr_size = std.mem.readIntLittle(u32, @ptrCast(stringSize));
  const casted: [*]u8 = @ptrFromInt(bytesPtr);
  allocator.free(casted[0..ptr_size]);
}

export fn __export_add(left: u8, right: u8, ) u8{
  const result = Guest.add(left, right);
  return result;
}

const Guest = struct {
  fn concat(left: []u8, right: []u8, ) []u8 {
    const buffer = alloc(left.len + right.len);
    @memcpy(buffer[0..left.len], left);
    @memcpy(buffer[left.len .. left.len + right.len], right);
    return buffer[0 .. left.len + right.len];
  }
  fn add(left: u8, right: u8, ) u8 {
    return left + right;
  }
};

comptime {
  @export(__export_concat, .{ .name = "concat" });
  @export(__export_add, .{ .name = "add" });
  @export(__post_return_concat, . { .name = "cabi_post_concat" });
}

pub fn main() void {}