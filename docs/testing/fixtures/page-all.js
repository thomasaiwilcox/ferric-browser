if (window.top !== window) {
    fetch("/__ferric_browser_userscript_subframe_hit__").catch(function() {});
}
