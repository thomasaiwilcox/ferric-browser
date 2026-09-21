if (window.top !== window) {
    fetch("/__rustbrowser_userscript_subframe_hit__").catch(function() {});
}
