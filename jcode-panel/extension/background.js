async function selectedText(tabId) {
  try {
    const [result] = await chrome.scripting.executeScript({
      target: { tabId },
      func: () => window.getSelection ? String(window.getSelection()) : ""
    });
    return result && result.result ? result.result : "";
  } catch (_err) {
    return "";
  }
}

async function report(tab) {
  if (!tab || !tab.id) return;
  const text = await selectedText(tab.id);
  try {
    await fetch("http://127.0.0.1:8765/", {
      method: "POST",
      headers: {"content-type": "application/json"},
      body: JSON.stringify({title: tab.title || "", url: tab.url || "", selectedText: text})
    });
  } catch (_err) {}
}

chrome.tabs.onActivated.addListener(async ({tabId}) => report(await chrome.tabs.get(tabId)));
chrome.tabs.onUpdated.addListener((_tabId, changeInfo, tab) => { if (changeInfo.status === "complete") report(tab); });
chrome.windows.onFocusChanged.addListener(async () => {
  const [tab] = await chrome.tabs.query({active: true, currentWindow: true});
  report(tab);
});
