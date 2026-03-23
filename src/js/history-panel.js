// ── History State ──────────────────────────────────────────────────────────

let selectedRuns = [];

// ── Refresh History List ──────────────────────────────────────────────────

async function refreshHistory() {
  try {
    const runs = await tauriListRuns();
    const tbody = document.getElementById('history-tbody');

    if (runs.length === 0) {
      tbody.innerHTML = '<tr><td colspan="6" class="empty-message">No saved runs yet. Run a simulation to get started.</td></tr>';
      return;
    }

    tbody.innerHTML = runs.map(run => {
      const date = new Date(run.timestamp).toLocaleString();
      const checked = selectedRuns.includes(run.id) ? 'checked' : '';
      return `
        <tr>
          <td><input type="checkbox" class="run-checkbox" data-id="${run.id}" ${checked} onchange="toggleRunSelection(this)" /></td>
          <td>${escapeHtml(run.run_name)}</td>
          <td>${date}</td>
          <td>${run.num_games.toLocaleString()}</td>
          <td>${run.player_count}</td>
          <td class="action-cell">
            <button class="btn-small" onclick="viewRun('${run.id}')">View</button>
            <button class="btn-small" onclick="exportRun('${run.id}', '${escapeHtml(run.run_name)}')">Export</button>
            <button class="btn-small btn-danger-small" onclick="deleteRun('${run.id}')">Delete</button>
          </td>
        </tr>`;
    }).join('');
  } catch (e) {
    console.error('Failed to load history:', e);
  }
}

// ── Selection & Compare ───────────────────────────────────────────────────

function toggleRunSelection(checkbox) {
  const runId = checkbox.dataset.id;

  if (checkbox.checked) {
    if (selectedRuns.length >= 2) {
      // Uncheck the oldest selection
      const oldId = selectedRuns.shift();
      const oldCheckbox = document.querySelector(`.run-checkbox[data-id="${oldId}"]`);
      if (oldCheckbox) oldCheckbox.checked = false;
    }
    selectedRuns.push(runId);
  } else {
    selectedRuns = selectedRuns.filter(id => id !== runId);
  }

  updateCompareButton();
}

function updateCompareButton() {
  const btn = document.getElementById('compare-btn');
  btn.disabled = selectedRuns.length !== 2;
}

async function compareRuns() {
  if (selectedRuns.length !== 2) return;

  try {
    const result = await tauriCompareRuns(selectedRuns[0], selectedRuns[1]);
    displayComparison(result);
  } catch (e) {
    alert('Compare failed: ' + e);
  }
}

function displayComparison(result) {
  const container = document.getElementById('comparison-result');
  container.classList.remove('hidden');

  let html = `
    <h3>Comparing: "${escapeHtml(result.run_a_name)}" vs "${escapeHtml(result.run_b_name)}"</h3>
    <table class="results-table comparison-table">
      <thead>
        <tr>
          <th>Metric</th>
          <th>Run A</th>
          <th>Run B</th>
          <th>Delta</th>
          <th>% Change</th>
        </tr>
      </thead>
      <tbody>`;

  for (const d of result.diffs) {
    const deltaClass = d.delta > 0 ? 'positive' : d.delta < 0 ? 'negative' : '';
    const deltaPrefix = d.delta > 0 ? '+' : '';
    html += `
      <tr>
        <td>${escapeHtml(d.name)}</td>
        <td>${formatNum(d.run_a)}</td>
        <td>${formatNum(d.run_b)}</td>
        <td class="${deltaClass}">${deltaPrefix}${formatNum(d.delta)}</td>
        <td class="${deltaClass}">${deltaPrefix}${d.percent_change.toFixed(1)}%</td>
      </tr>`;
  }

  html += '</tbody></table>';
  container.innerHTML = html;
}

// ── View Run ──────────────────────────────────────────────────────────────

async function viewRun(runId) {
  try {
    const summary = await tauriGetRun(runId);
    currentSummary = summary;
    displayResults(summary);
    switchTab('results');
  } catch (e) {
    alert('Failed to load run: ' + e);
  }
}

// ── Export Run ─────────────────────────────────────────────────────────────

async function exportRun(runId, runName) {
  try {
    const csv = await tauriExportRunCsv(runId);
    // Create a downloadable blob
    const blob = new Blob([csv], { type: 'text/csv' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${runName.replace(/[^a-zA-Z0-9]/g, '_')}.csv`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  } catch (e) {
    alert('Export failed: ' + e);
  }
}

// ── Delete Run ────────────────────────────────────────────────────────────

async function deleteRun(runId) {
  if (!confirm('Delete this run? This cannot be undone.')) return;

  try {
    await tauriDeleteRun(runId);
    selectedRuns = selectedRuns.filter(id => id !== runId);
    updateCompareButton();
    await refreshHistory();
  } catch (e) {
    alert('Delete failed: ' + e);
  }
}

// ── Utilities ─────────────────────────────────────────────────────────────

function escapeHtml(str) {
  const div = document.createElement('div');
  div.textContent = str;
  return div.innerHTML;
}

function formatNum(n) {
  if (Number.isInteger(n)) return n.toLocaleString();
  return n.toFixed(2);
}
