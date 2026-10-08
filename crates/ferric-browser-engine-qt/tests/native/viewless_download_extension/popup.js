const downloadUrl = new URL(location.href).searchParams.get("downloadUrl");

chrome.runtime.sendMessage(
    { kind: "ferric-viewless-download", url: downloadUrl },
    response => {
        document.documentElement.dataset.result =
            !chrome.runtime.lastError && response && response.started
                ? "started" : "refused";
    });
