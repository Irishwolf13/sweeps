// ── Tab Switching ─────────────────────────────────────────────────────────

function switchTab(tabName) {
  document.querySelectorAll('.tab-btn').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.tab === tabName);
  });
  document.querySelectorAll('.tab-content').forEach(content => {
    content.classList.toggle('active', content.id === 'tab-' + tabName);
  });
  if (tabName === 'history') {
    refreshHistory();
  }
}

document.querySelectorAll('.tab-btn').forEach(btn => {
  btn.addEventListener('click', () => switchTab(btn.dataset.tab));
});

// ── Simulation Execution ──────────────────────────────────────────────────

let currentSummary = null;

async function runSimulation() {
  const runBtn = document.getElementById('run-btn');
  const progressContainer = document.getElementById('progress-container');
  const progressFill = document.getElementById('progress-fill');
  const progressText = document.getElementById('progress-text');

  // Build config
  const config = buildConfigFromUI();
  const numGames = parseInt(document.getElementById('num-games').value);

  // Auto-generate run name if empty
  const now = new Date();
  const defaultName = `Run ${now.toLocaleDateString()} ${now.toLocaleTimeString()}`;
  const runName = document.getElementById('run-name').value.trim() || defaultName;

  // Frontend validation
  const validationError = validateConfig(config);
  if (validationError) {
    alert('Configuration error: ' + validationError);
    return;
  }

  // Disable button, show progress
  runBtn.disabled = true;
  runBtn.textContent = 'Running...';
  progressContainer.classList.remove('hidden');
  progressFill.style.width = '0%';
  progressText.textContent = '0%';

  // Poll progress
  const progressInterval = setInterval(async () => {
    try {
      const [current, total, running] = await tauriGetProgress();
      if (total > 0) {
        const pct = Math.round((current / total) * 100);
        progressFill.style.width = pct + '%';
        progressText.textContent = `${pct}% (${current.toLocaleString()} / ${total.toLocaleString()})`;
      }
    } catch (e) {
      // Ignore polling errors
    }
  }, 250);

  try {
    const saveDetailed = document.getElementById('save-detailed').checked;
    const summary = await tauriRunSimulation(config, numGames, runName, saveDetailed);
    currentSummary = summary;
    displayResults(summary);
    switchTab('results');
  } catch (e) {
    alert('Simulation failed: ' + e);
  } finally {
    clearInterval(progressInterval);
    runBtn.disabled = false;
    runBtn.textContent = 'Run Simulation';
    // Keep progress showing 100% briefly
    progressFill.style.width = '100%';
    progressText.textContent = '100%';
    setTimeout(() => {
      progressContainer.classList.add('hidden');
    }, 2000);
  }
}

// ── Results Display ───────────────────────────────────────────────────────

function displayResults(summary) {
  const placeholder = document.getElementById('results-placeholder');
  const content = document.getElementById('results-content');

  placeholder.classList.add('hidden');
  content.classList.remove('hidden');

  const playerCount = summary.config.player_count;

  const playerLabels = Array.from({length: playerCount}, (_, i) => `P${i+1}`);

  // Build charts
  const scoreRangeChart = rangeChart({
    labels: playerLabels,
    avg: summary.avg_total_score,
    median: summary.median_total_score,
    min: summary.min_total_score,
    max: summary.max_total_score,
    title: `Score Distribution (${summary.config.player_count * (summary.config.round_multiplier || 1)} rounds total)`,
    width: 560,
  });

  const winRateChart = barChart({
    labels: playerLabels,
    values: summary.win_rates,
    title: 'Win Rates',
    unit: '%',
    width: 340,
    height: 240,
  });

  const avgScoreChart = barChart({
    labels: playerLabels,
    values: summary.avg_total_score,
    min: summary.min_total_score,
    max: summary.max_total_score,
    title: 'Average Total Score',
    unit: ' pts',
    width: 340,
    height: 240,
  });

  const elimChart = barChart({
    labels: playerLabels,
    values: summary.player_summaries.map(p => p.avg_eliminations),
    title: 'Avg Eliminations / Round',
    width: 340,
    height: 240,
    decimals: 2,
  });

  const cardsChart = barChart({
    labels: playerLabels,
    values: summary.avg_cards_remaining,
    title: 'Avg Cards Remaining / Round',
    width: 340,
    height: 240,
  });

  const bellCurveChart = summary.score_histograms ? distributionChart({
    labels: playerLabels,
    histograms: summary.score_histograms,
    title: 'Score Distribution (Bell Curve)',
    width: 700,
    height: 300,
  }) : '';

  const cardsDistChart = summary.cards_remaining_histograms ? distributionChart({
    labels: playerLabels,
    histograms: summary.cards_remaining_histograms,
    title: 'Cards Remaining Distribution (Per Round)',
    xLabel: 'Cards Remaining',
    width: 700,
    height: 300,
  }) : '';

  let html = `
    <h2 class="results-title">${summary.run_name} — ${summary.num_games.toLocaleString()} games</h2>

    ${bellCurveChart ? `<div class="chart-row"><div class="chart-container chart-wide">${bellCurveChart}</div></div>` : ''}
    <div class="chart-row">
      <div class="chart-container">${scoreRangeChart}</div>
    </div>
    <div class="chart-row">
      <div class="chart-container">${winRateChart}</div>
      <div class="chart-container">${avgScoreChart}</div>
    </div>
    <div class="chart-row">
      <div class="chart-container">${elimChart}</div>
      <div class="chart-container">${cardsChart}</div>
    </div>
    ${cardsDistChart ? `<div class="chart-row"><div class="chart-container chart-wide">${cardsDistChart}</div></div>` : ''}

    <div class="stat-cards">
      <div class="stat-card">
        <h3>Game Length</h3>
        <div class="stat-row"><span class="label">Avg turns/round</span><span class="value">${summary.avg_turns_per_round.toFixed(1)}</span></div>
        <div class="stat-row"><span class="label">Avg eliminations/round</span><span class="value">${summary.avg_eliminations_per_round.toFixed(2)}</span></div>
        <div class="stat-row"><span class="label">Avg score/round</span><span class="value">${summary.avg_score_per_round.toFixed(1)}</span></div>
      </div>
      <div class="stat-card">
        <h3>Win Rates</h3>
        ${summary.win_rates.map((r, i) => `
          <div class="stat-row"><span class="label">Player ${i+1}</span><span class="value">${r.toFixed(1)}%</span></div>
        `).join('')}
        <div class="stat-row"><span class="label">First mover advantage</span><span class="value ${summary.first_mover_advantage > 0 ? 'positive' : 'negative'}">${summary.first_mover_advantage > 0 ? '+' : ''}${summary.first_mover_advantage.toFixed(1)}%</span></div>
      </div>
      <div class="stat-card">
        <h3>Deck Health</h3>
        <div class="stat-row"><span class="label">Draw pile exhaustion</span><span class="value">${summary.draw_pile_exhaustion_rate.toFixed(1)}%</span></div>
        <div class="stat-row"><span class="label">Avg draw pile remaining</span><span class="value">${summary.avg_draw_pile_remaining.toFixed(1)} cards</span></div>
        <div class="stat-row"><span class="label">Effective deck usage</span><span class="value">${summary.effective_deck_usage.toFixed(1)}%</span></div>
        <div class="stat-row"><span class="label">Round completion rate</span><span class="value">${summary.round_completion_rate.toFixed(1)}%</span></div>
      </div>
      <div class="stat-card">
        <h3>Scoring</h3>
        <div class="stat-row"><span class="label">Went out first rate</span><span class="value">${summary.went_out_first_rate.toFixed(1)}%</span></div>
        <div class="stat-row"><span class="label">Cleared all rate</span><span class="value">${summary.cleared_all_rate.toFixed(1)}%</span></div>
      </div>
    </div>

    <div class="config-section">
      <h2>Per-Player Breakdown</h2>
      <table class="results-table">
        <thead>
          <tr>
            <th>Metric</th>
            ${Array.from({length: playerCount}, (_, i) => `<th>Player ${i+1}</th>`).join('')}
          </tr>
        </thead>
        <tbody>
          <tr><td>Avg Score</td>${summary.avg_total_score.map(s => `<td>${s.toFixed(1)}</td>`).join('')}</tr>
          <tr><td>Median Score</td>${summary.median_total_score.map(s => `<td>${s}</td>`).join('')}</tr>
          <tr><td>Std Dev</td>${summary.stddev_total_score.map(s => `<td>${s.toFixed(1)}</td>`).join('')}</tr>
          <tr><td>Min Score</td>${summary.min_total_score.map(s => `<td>${s}</td>`).join('')}</tr>
          <tr><td>Max Score</td>${summary.max_total_score.map(s => `<td>${s}</td>`).join('')}</tr>
          <tr><td>Win Rate</td>${summary.win_rates.map(r => `<td>${r.toFixed(1)}%</td>`).join('')}</tr>
          <tr><td>Avg Eliminations</td>${summary.player_summaries.map(p => `<td>${p.avg_eliminations.toFixed(2)}</td>`).join('')}</tr>
          <tr><td>Avg Cards Remaining</td>${summary.avg_cards_remaining.map(c => `<td>${c.toFixed(1)}</td>`).join('')}</tr>
          <tr><td>Went Out First</td>${summary.player_summaries.map(p => `<td>${p.went_out_first_count}</td>`).join('')}</tr>
          <tr><td>Cleared All</td>${summary.player_summaries.map(p => `<td>${p.cleared_all_count}</td>`).join('')}</tr>
        </tbody>
      </table>
    </div>
  `;

  content.innerHTML = html;
}

// ── Validation ────────────────────────────────────────────────────────────

function validateConfig(config) {
  if (config.deck.neg_min > 0) {
    return 'Negative range minimum must be <= 0';
  }
  if (config.deck.pos_max < 0) {
    return 'Positive range maximum must be >= 0';
  }
  if (config.deck.neg_min > config.deck.pos_max) {
    return 'Negative min must be less than positive max';
  }

  const totalCards = config.deck.card_quantities.reduce((sum, [_, count]) => sum + count, 0)
    + config.deck.wild_count;
  const needed = config.player_count * 16 + 20;

  if (totalCards < needed) {
    return `Deck has ${totalCards} cards but ${config.player_count} players need at least ${needed} (${config.player_count}x16 + 20 for draw pile)`;
  }

  if (config.players.length !== config.player_count) {
    return 'Player config count mismatch';
  }

  return null;
}

// ── Initialize ────────────────────────────────────────────────────────────

document.addEventListener('DOMContentLoaded', () => {
  buildCardQuantityTable();
  buildPlayerPanels();
});
