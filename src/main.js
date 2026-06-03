import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

const appWindow = getCurrentWindow();
const input = document.querySelector("#search-input");
const status = document.querySelector("#status");
const results = document.querySelector("#results");
const closeButton = document.querySelector("#close-button");
const tabButtons = document.querySelectorAll(".tab-button");
const searchView = document.querySelector("#search-view");
const scheduleView = document.querySelector("#schedule-view");
const scheduleForm = document.querySelector("#schedule-form");
const scheduleTitle = document.querySelector("#schedule-title");
const scheduleDate = document.querySelector("#schedule-date");
const scheduleStart = document.querySelector("#schedule-start");
const scheduleEnd = document.querySelector("#schedule-end");
const scheduleNote = document.querySelector("#schedule-note");
const scheduleStatus = document.querySelector("#schedule-status");
const scheduleList = document.querySelector("#schedule-list");

let desktopItems = [];
let filteredItems = [];
let activeIndex = 0;
let schedules = [];

function todayString() {
  const date = new Date();
  const month = `${date.getMonth() + 1}`.padStart(2, "0");
  const day = `${date.getDate()}`.padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

function normalize(value) {
  return value.trim().toLocaleLowerCase("zh-CN");
}

function itemMatches(item, keyword) {
  if (!keyword) {
    return true;
  }
  return normalize(item.name).includes(keyword) || normalize(item.path ?? "").includes(keyword);
}

function renderResults() {
  results.replaceChildren();

  filteredItems.slice(0, 30).forEach((item, index) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = `result-item${index === activeIndex ? " active" : ""}`;
    button.innerHTML = `<strong>${item.name}</strong><span class="result-meta">${item.path ?? "桌面项目"}</span>`;
    button.addEventListener("pointerdown", (event) => {
      event.preventDefault();
      highlightItem(item);
    });
    button.addEventListener("dblclick", (event) => {
      event.preventDefault();
      highlightItem(item);
    });

    const li = document.createElement("li");
    li.append(button);
    results.append(li);
  });

  status.textContent = filteredItems.length === 0 ? "没有匹配的桌面图标" : `找到 ${filteredItems.length} 个匹配项`;
}

function refreshFilter() {
  const keyword = normalize(input.value);
  filteredItems = desktopItems.filter((item) => itemMatches(item, keyword));
  activeIndex = 0;
  renderResults();
}

async function loadDesktopItems() {
  try {
    desktopItems = await invoke("list_desktop_items");
    refreshFilter();
  } catch (error) {
    status.textContent = `读取桌面图标失败：${error}`;
  }
}

async function loadSchedules() {
  try {
    schedules = await invoke("list_schedules", { date: scheduleDate.value || todayString() });
    renderSchedules();
  } catch (error) {
    scheduleStatus.textContent = `读取今日计划失败：${error}`;
  }
}

function renderSchedules() {
  scheduleList.replaceChildren();

  if (schedules.length === 0) {
    scheduleStatus.textContent = "今天还没有计划";
    return;
  }

  scheduleStatus.textContent = `今日计划 ${schedules.length} 项`;
  schedules.forEach((item) => {
    const li = document.createElement("li");
    li.className = `schedule-item ${item.status === "done" ? "done" : ""}`;

    const time = [item.start_time, item.end_time].filter(Boolean).join(" - ");
    const content = document.createElement("div");
    content.className = "schedule-content";

    const title = document.createElement("strong");
    title.textContent = item.title;

    const meta = document.createElement("span");
    meta.textContent = `${time || "未设置时间"}${item.note ? ` · ${item.note}` : ""}`;

    const actions = document.createElement("div");
    actions.className = "schedule-actions";

    const toggleButton = document.createElement("button");
    toggleButton.type = "button";
    toggleButton.textContent = item.status === "done" ? "撤销" : "完成";
    toggleButton.addEventListener("click", () => toggleSchedule(item.id));

    const deleteButton = document.createElement("button");
    deleteButton.type = "button";
    deleteButton.textContent = "删除";
    deleteButton.addEventListener("click", () => deleteScheduleItem(item.id));

    content.append(title, meta);
    actions.append(toggleButton, deleteButton);
    li.append(content, actions);
    scheduleList.append(li);
  });
}

async function createSchedule(event) {
  event.preventDefault();
  const title = scheduleTitle.value.trim();
  if (!title) {
    scheduleStatus.textContent = "计划内容不能为空";
    return;
  }

  try {
    await invoke("create_schedule", {
      input: {
        title,
        date: scheduleDate.value || todayString(),
        start_time: scheduleStart.value,
        end_time: scheduleEnd.value,
        note: scheduleNote.value.trim(),
      },
    });
    scheduleTitle.value = "";
    scheduleNote.value = "";
    await loadSchedules();
  } catch (error) {
    scheduleStatus.textContent = `添加计划失败：${error}`;
  }
}

async function toggleSchedule(id) {
  try {
    await invoke("toggle_schedule_done", { id });
    await loadSchedules();
  } catch (error) {
    scheduleStatus.textContent = `更新计划失败：${error}`;
  }
}

async function deleteScheduleItem(id) {
  try {
    await invoke("delete_schedule", { id });
    await loadSchedules();
  } catch (error) {
    scheduleStatus.textContent = `删除计划失败：${error}`;
  }
}

function switchView(view) {
  const isSchedule = view === "schedule";
  searchView.classList.toggle("active", !isSchedule);
  scheduleView.classList.toggle("active", isSchedule);
  tabButtons.forEach((button) => button.classList.toggle("active", button.dataset.view === view));

  if (isSchedule) {
    loadSchedules();
    scheduleTitle.focus();
  } else {
    input.focus();
  }
}

async function highlightItem(item) {
  try {
    status.textContent = `正在高亮：${item.name}`;
    await invoke("highlight_desktop_item", { name: item.name, path: item.path ?? null });
    // 高亮后保持窗口打开，方便连续查找；仅 Esc 或点 × 手动关闭。
    status.textContent = `已高亮：${item.name}（按 Esc 或 × 关闭）`;
  } catch (error) {
    status.textContent = `高亮失败：${error}`;
  }
}

async function hideLauncher() {
  try {
    await appWindow.hide();
  } catch (error) {
    status.textContent = `关闭窗口失败：${error}`;
  }
}

closeButton.addEventListener("pointerdown", (event) => {
  event.preventDefault();
  hideLauncher();
});
closeButton.addEventListener("click", hideLauncher);
input.addEventListener("input", refreshFilter);
scheduleForm.addEventListener("submit", createSchedule);
scheduleDate.addEventListener("change", loadSchedules);
tabButtons.forEach((button) => button.addEventListener("click", () => switchView(button.dataset.view)));

// Esc 挂在 window 上，无论焦点在输入框、结果项还是 × 按钮都能关闭。
window.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    event.preventDefault();
    hideLauncher();
  }
});

input.addEventListener("keydown", async (event) => {
  if (event.key === "ArrowDown") {
    activeIndex = Math.min(activeIndex + 1, Math.max(filteredItems.length - 1, 0));
    renderResults();
    return;
  }

  if (event.key === "ArrowUp") {
    activeIndex = Math.max(activeIndex - 1, 0);
    renderResults();
    return;
  }

  if (event.key === "Enter" && filteredItems[activeIndex]) {
    await highlightItem(filteredItems[activeIndex]);
  }
});

window.addEventListener("DOMContentLoaded", () => {
  scheduleDate.value = todayString();
  loadDesktopItems();
});
