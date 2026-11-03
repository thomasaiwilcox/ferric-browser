// Optional Node check of the actual script builder and test instrumentation.
// DOM/candidate equivalence is covered by T043 on the qualified Qt runner.
const fs = require('node:fs');
const vm = require('node:vm');
const context = vm.createContext({});
const builder = fs.readFileSync('crates/ferric-browser-engine-qt/qml/scripts/PageInteractionScripts.js', 'utf8');
vm.runInContext(builder.replace(/^\.pragma library\s*\n/m, ''), context);
const test = fs.readFileSync('crates/ferric-browser-engine-qt/tests/qml-perf/tst_hint_performance.qml', 'utf8');
const start = test.indexOf('    function replaceOnce(');
const end = test.indexOf('    function test_T043_samples_data(');
if (start < 0 || end <= start) throw new Error('Missing benchmark instrumentation');
vm.runInContext(test.slice(start, end), context);
for (const family of ['buttons', 'links', 'inputs', 'all', 'regions', 'surfaces']) {
    const source = vm.runInContext(`profiledCollectorScript(hintCollector('${family}'))`, context);
    new vm.Script(source);
}
let rejected = false;
try {
    vm.runInContext("profiledCollectorScript(hintCollector('buttons').replace('if(ancestors&&ancestors.has(node)){', 'if(false){'))", context);
} catch (error) {
    if (!error.message.includes('anchor mismatch')) throw error;
    rejected = true;
}
if (!rejected) throw new Error('Stale instrumentation was silently accepted');
console.log('PASS: six collector families compile; stale instrumentation fails closed');
