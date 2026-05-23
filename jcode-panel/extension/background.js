async function selectionDetails(tabId) {
  try {
    const [result] = await chrome.scripting.executeScript({
      target: { tabId },
      func: () => {
        const selection = window.getSelection ? window.getSelection() : null;
        const selectedText = selection ? String(selection) : "";
        if (!selection || selection.rangeCount === 0 || !selectedText.trim()) {
          return { selectedText: "", selectionLine: null, selectionContext: "" };
        }
        const range = selection.getRangeAt(0);
        const container = range.startContainer;
        const element = container.nodeType === Node.ELEMENT_NODE
          ? container
          : container.parentElement;
        const block = element && element.closest
          ? element.closest('p,li,pre,code,blockquote,article,section,main,div,td,th,h1,h2,h3,h4,h5,h6')
          : null;
        const contextText = (block && block.innerText ? block.innerText : (element && element.innerText) || selectedText)
          .replace(/\s+/g, ' ')
          .trim()
          .slice(0, 500);
        let selectionLine = null;
        try {
          const preRange = document.createRange();
          preRange.selectNodeContents(document.body);
          preRange.setEnd(range.startContainer, range.startOffset);
          selectionLine = String(preRange.toString()).split(/\n/).length;
        } catch (_err) {}
        return { selectedText, selectionLine, selectionContext: contextText };
      }
    });
    return result && result.result ? result.result : { selectedText: "", selectionLine: null, selectionContext: "" };
  } catch (_err) {
    return { selectedText: "", selectionLine: null, selectionContext: "" };
  }
}

async function report(tab) {
  if (!tab || !tab.id) return;
  const details = await selectionDetails(tab.id);
  try {
    await fetch("http://127.0.0.1:8765/", {
      method: "POST",
      headers: {"content-type": "application/json"},
      body: JSON.stringify({
        title: tab.title || "",
        url: tab.url || "",
        selectedText: details.selectedText || "",
        selectionLine: details.selectionLine || null,
        selectionContext: details.selectionContext || ""
      })
    });
  } catch (_err) {}
}

chrome.tabs.onActivated.addListener(async ({tabId}) => report(await chrome.tabs.get(tabId)));
chrome.tabs.onUpdated.addListener((_tabId, changeInfo, tab) => { if (changeInfo.status === "complete") report(tab); });
chrome.windows.onFocusChanged.addListener(async () => {
  const [tab] = await chrome.tabs.query({active: true, currentWindow: true});
  report(tab);
});
