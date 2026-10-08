chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
    if (!message || message.kind !== "ferric-viewless-download"
            || typeof message.url !== "string"
            || !/^http:\/\/127\.0\.0\.1(?::\d+)?\/(?:viewless-download|versioning\/archive-download)$/.test(message.url)) {
        sendResponse({ started: false });
        return false;
    }

    chrome.downloads.download({ url: message.url, saveAs: false }, downloadId => {
        sendResponse({
            started: !chrome.runtime.lastError && Number.isInteger(downloadId)
        });
    });
    return true;
});
