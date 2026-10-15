const fs = require('fs');
const assert = require('assert/strict');

const moduleBytes = fs.readFileSync(process.argv[2]);
const moduleObject = new WebAssembly.Module(moduleBytes);
assert.deepEqual(WebAssembly.Module.imports(moduleObject), []);
const instance = new WebAssembly.Instance(moduleObject);
const e = instance.exports;
const trace = [];
function check(step, expected) {
    const count = e.raw_ctor_count();
    trace.push({step, count});
    assert.equal(count, expected);
}

check('instantiate core module', 0);
let ptr = e.cabi_realloc(0, 0, 4, 16);
assert(ptr > 0);
check('allocate before initialization', 0);
ptr = e.cabi_realloc(ptr, 16, 4, 32);
assert(ptr > 0);
check('reallocate before initialization', 0);
e._initialize();
check('call _initialize', 1);
assert.equal(e['the:test/i#ctor-runs'](), 1);
check('call first component export', 1);
assert.equal(e['the:test/i#ctor-runs'](), 1);
check('call second component export', 1);
ptr = e.cabi_realloc(ptr, 32, 4, 64);
assert(ptr > 0);
check('reallocate after initialization', 1);
assert.throws(() => e._initialize(), WebAssembly.RuntimeError);
check('second _initialize traps', 1);
console.log(JSON.stringify({imports: [], trace}, null, 2));
