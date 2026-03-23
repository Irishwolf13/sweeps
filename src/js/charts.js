// ── SVG Chart Utilities ──────────────────────────────────────────────────

const CHART_COLORS = ['#e94560', '#4fc3f7', '#69f0ae', '#ffab40', '#b388ff', '#ff8a80'];

/**
 * Bar chart with optional whiskers (min/max) and value labels.
 * @param {Object} opts
 * @param {string[]} opts.labels - Category labels (e.g. player names)
 * @param {number[]} opts.values - Bar values
 * @param {number[]} [opts.min] - Min whisker values
 * @param {number[]} [opts.max] - Max whisker values
 * @param {string} [opts.title] - Chart title
 * @param {string} [opts.unit] - Unit suffix for labels (e.g. '%', ' pts')
 * @param {number} [opts.width] - SVG width
 * @param {number} [opts.height] - SVG height
 * @param {number} [opts.decimals] - Decimal places for labels
 * @returns {string} SVG markup
 */
function barChart(opts) {
  const {
    labels, values, min, max, title,
    unit = '', width = 500, height = 260, decimals = 1
  } = opts;

  const n = labels.length;
  const pad = { top: title ? 40 : 20, right: 20, bottom: 40, left: 50 };
  const plotW = width - pad.left - pad.right;
  const plotH = height - pad.top - pad.bottom;

  // Compute range
  const allVals = [...values];
  if (min) allVals.push(...min);
  if (max) allVals.push(...max);
  let dataMin = Math.min(...allVals);
  let dataMax = Math.max(...allVals);
  // Ensure range includes 0 for context
  if (dataMin > 0) dataMin = 0;
  if (dataMax < 0) dataMax = 0;
  const range = dataMax - dataMin || 1;
  const margin = range * 0.1;
  const scaleMin = dataMin - margin;
  const scaleMax = dataMax + margin;
  const scaleRange = scaleMax - scaleMin;

  const yScale = v => pad.top + plotH - ((v - scaleMin) / scaleRange) * plotH;
  const barW = Math.min(plotW / n * 0.6, 60);
  const gap = plotW / n;

  let svg = `<svg width="${width}" height="${height}" class="chart-svg">`;

  // Title
  if (title) {
    svg += `<text x="${width / 2}" y="18" class="chart-title">${title}</text>`;
  }

  // Y-axis grid lines
  const nTicks = 5;
  for (let i = 0; i <= nTicks; i++) {
    const val = scaleMin + (scaleRange * i / nTicks);
    const y = yScale(val);
    svg += `<line x1="${pad.left}" y1="${y}" x2="${width - pad.right}" y2="${y}" class="chart-grid"/>`;
    svg += `<text x="${pad.left - 6}" y="${y + 4}" class="chart-axis-label" text-anchor="end">${val.toFixed(0)}</text>`;
  }

  // Zero line
  if (scaleMin < 0 && scaleMax > 0) {
    const y0 = yScale(0);
    svg += `<line x1="${pad.left}" y1="${y0}" x2="${width - pad.right}" y2="${y0}" class="chart-zero"/>`;
  }

  // Bars
  for (let i = 0; i < n; i++) {
    const cx = pad.left + gap * i + gap / 2;
    const val = values[i];
    const y0 = yScale(0);
    const yVal = yScale(val);
    const barX = cx - barW / 2;
    const color = CHART_COLORS[i % CHART_COLORS.length];

    // Bar (handles negative values)
    const barTop = Math.min(y0, yVal);
    const barH = Math.abs(y0 - yVal);
    svg += `<rect x="${barX}" y="${barTop}" width="${barW}" height="${Math.max(barH, 1)}" fill="${color}" rx="3" class="chart-bar"/>`;

    // Whiskers (min/max)
    if (min && max) {
      const yMin = yScale(min[i]);
      const yMax = yScale(max[i]);
      const whiskerW = barW * 0.4;
      svg += `<line x1="${cx}" y1="${yMin}" x2="${cx}" y2="${yMax}" stroke="${color}" stroke-width="1.5" opacity="0.5"/>`;
      svg += `<line x1="${cx - whiskerW/2}" y1="${yMin}" x2="${cx + whiskerW/2}" y2="${yMin}" stroke="${color}" stroke-width="2" opacity="0.6"/>`;
      svg += `<line x1="${cx - whiskerW/2}" y1="${yMax}" x2="${cx + whiskerW/2}" y2="${yMax}" stroke="${color}" stroke-width="2" opacity="0.6"/>`;
    }

    // Value label
    const labelY = val >= 0 ? yVal - 6 : yVal + 14;
    svg += `<text x="${cx}" y="${labelY}" class="chart-value">${val.toFixed(decimals)}${unit}</text>`;

    // Category label
    svg += `<text x="${cx}" y="${height - pad.bottom + 18}" class="chart-cat-label">${labels[i]}</text>`;
  }

  svg += '</svg>';
  return svg;
}

/**
 * Grouped bar chart — multiple series side by side.
 * @param {Object} opts
 * @param {string[]} opts.labels - Category labels
 * @param {Object[]} opts.series - Array of { name, values, color? }
 * @param {string} [opts.title]
 * @param {string} [opts.unit]
 * @param {number} [opts.width]
 * @param {number} [opts.height]
 * @param {number} [opts.decimals]
 * @returns {string} SVG markup
 */
function groupedBarChart(opts) {
  const {
    labels, series, title,
    unit = '', width = 500, height = 280, decimals = 1
  } = opts;

  const n = labels.length;
  const m = series.length;
  const pad = { top: title ? 45 : 25, right: 20, bottom: 55, left: 50 };
  const plotW = width - pad.left - pad.right;
  const plotH = height - pad.top - pad.bottom;

  const allVals = series.flatMap(s => s.values);
  let dataMin = Math.min(0, ...allVals);
  let dataMax = Math.max(0, ...allVals);
  const range = dataMax - dataMin || 1;
  const margin = range * 0.1;
  const scaleMin = dataMin - margin;
  const scaleMax = dataMax + margin;
  const scaleRange = scaleMax - scaleMin;

  const yScale = v => pad.top + plotH - ((v - scaleMin) / scaleRange) * plotH;
  const groupW = plotW / n;
  const barW = Math.min(groupW * 0.7 / m, 40);

  let svg = `<svg width="${width}" height="${height}" class="chart-svg">`;

  if (title) {
    svg += `<text x="${width / 2}" y="18" class="chart-title">${title}</text>`;
  }

  // Grid
  const nTicks = 5;
  for (let i = 0; i <= nTicks; i++) {
    const val = scaleMin + (scaleRange * i / nTicks);
    const y = yScale(val);
    svg += `<line x1="${pad.left}" y1="${y}" x2="${width - pad.right}" y2="${y}" class="chart-grid"/>`;
    svg += `<text x="${pad.left - 6}" y="${y + 4}" class="chart-axis-label" text-anchor="end">${val.toFixed(0)}</text>`;
  }

  // Zero line
  if (scaleMin < 0 && scaleMax > 0) {
    const y0 = yScale(0);
    svg += `<line x1="${pad.left}" y1="${y0}" x2="${width - pad.right}" y2="${y0}" class="chart-zero"/>`;
  }

  // Bars
  for (let i = 0; i < n; i++) {
    const groupX = pad.left + groupW * i;
    const totalBarsW = barW * m;
    const startX = groupX + (groupW - totalBarsW) / 2;

    for (let j = 0; j < m; j++) {
      const val = series[j].values[i];
      const color = series[j].color || CHART_COLORS[j % CHART_COLORS.length];
      const cx = startX + barW * j + barW / 2;
      const y0 = yScale(0);
      const yVal = yScale(val);
      const barTop = Math.min(y0, yVal);
      const barH = Math.abs(y0 - yVal);

      svg += `<rect x="${cx - barW/2 + 1}" y="${barTop}" width="${barW - 2}" height="${Math.max(barH, 1)}" fill="${color}" rx="2" class="chart-bar"/>`;

      // Value label (only if bars aren't too cramped)
      if (barW >= 20) {
        const labelY = val >= 0 ? yVal - 4 : yVal + 12;
        svg += `<text x="${cx}" y="${labelY}" class="chart-value" font-size="9">${val.toFixed(decimals)}${unit}</text>`;
      }
    }

    // Category label
    const cx = groupX + groupW / 2;
    svg += `<text x="${cx}" y="${height - pad.bottom + 18}" class="chart-cat-label">${labels[i]}</text>`;
  }

  // Legend
  const legendY = height - 12;
  const legendStartX = pad.left;
  for (let j = 0; j < m; j++) {
    const lx = legendStartX + j * 110;
    const color = series[j].color || CHART_COLORS[j % CHART_COLORS.length];
    svg += `<rect x="${lx}" y="${legendY - 8}" width="10" height="10" fill="${color}" rx="2"/>`;
    svg += `<text x="${lx + 14}" y="${legendY}" class="chart-legend-label">${series[j].name}</text>`;
  }

  svg += '</svg>';
  return svg;
}

/**
 * Horizontal bullet/range chart — shows min, avg, median, max per player.
 * Good for showing score distributions.
 */
function rangeChart(opts) {
  const {
    labels, avg, median, min, max, title,
    width = 500, height = null
  } = opts;

  const n = labels.length;
  const rowH = 40;
  const pad = { top: title ? 40 : 20, right: 30, bottom: 30, left: 80 };
  const h = (height || pad.top + n * rowH + pad.bottom);
  const plotW = width - pad.left - pad.right;

  const allVals = [...min, ...max];
  const dataMin = Math.min(...allVals);
  const dataMax = Math.max(...allVals);
  const range = dataMax - dataMin || 1;
  const margin = range * 0.05;
  const scaleMin = dataMin - margin;
  const scaleMax = dataMax + margin;
  const scaleRange = scaleMax - scaleMin;

  const xScale = v => pad.left + ((v - scaleMin) / scaleRange) * plotW;

  let svg = `<svg width="${width}" height="${h}" class="chart-svg">`;

  if (title) {
    svg += `<text x="${width / 2}" y="18" class="chart-title">${title}</text>`;
  }

  // X-axis ticks
  const nTicks = 6;
  for (let i = 0; i <= nTicks; i++) {
    const val = scaleMin + (scaleRange * i / nTicks);
    const x = xScale(val);
    svg += `<line x1="${x}" y1="${pad.top}" x2="${x}" y2="${h - pad.bottom}" class="chart-grid"/>`;
    svg += `<text x="${x}" y="${h - pad.bottom + 16}" class="chart-axis-label">${val.toFixed(0)}</text>`;
  }

  // Zero line
  if (scaleMin < 0 && scaleMax > 0) {
    const x0 = xScale(0);
    svg += `<line x1="${x0}" y1="${pad.top}" x2="${x0}" y2="${h - pad.bottom}" class="chart-zero"/>`;
  }

  // Rows
  for (let i = 0; i < n; i++) {
    const cy = pad.top + i * rowH + rowH / 2;
    const color = CHART_COLORS[i % CHART_COLORS.length];

    // Label
    svg += `<text x="${pad.left - 8}" y="${cy + 4}" class="chart-row-label" text-anchor="end">${labels[i]}</text>`;

    // Range line (min to max)
    svg += `<line x1="${xScale(min[i])}" y1="${cy}" x2="${xScale(max[i])}" y2="${cy}" stroke="${color}" stroke-width="2" opacity="0.4"/>`;
    // Min/max caps
    svg += `<line x1="${xScale(min[i])}" y1="${cy-6}" x2="${xScale(min[i])}" y2="${cy+6}" stroke="${color}" stroke-width="2" opacity="0.5"/>`;
    svg += `<line x1="${xScale(max[i])}" y1="${cy-6}" x2="${xScale(max[i])}" y2="${cy+6}" stroke="${color}" stroke-width="2" opacity="0.5"/>`;

    // Avg bar
    const avgX = xScale(avg[i]);
    svg += `<circle cx="${avgX}" cy="${cy}" r="6" fill="${color}" opacity="0.9"/>`;
    svg += `<text x="${avgX}" y="${cy - 10}" class="chart-value" font-size="10">avg ${avg[i].toFixed(1)}</text>`;

    // Median marker
    const medX = xScale(median[i]);
    svg += `<rect x="${medX - 2}" y="${cy - 8}" width="4" height="16" fill="white" opacity="0.7" rx="1"/>`;
  }

  svg += '</svg>';
  return svg;
}

/**
 * Overlaid distribution curves (bell curves) for multiple players.
 * Each dataset is an array of [score, count] histogram buckets.
 *
 * @param {Object} opts
 * @param {string[]} opts.labels - Player labels
 * @param {Array<Array<[number, number]>>} opts.histograms - Per-player histogram data
 * @param {string} [opts.title]
 * @param {number} [opts.width]
 * @param {number} [opts.height]
 * @returns {string} SVG markup
 */
function distributionChart(opts) {
  const {
    labels, histograms, title,
    width = 700, height = 320
  } = opts;

  const pad = { top: title ? 45 : 25, right: 30, bottom: 50, left: 55 };
  const plotW = width - pad.left - pad.right;
  const plotH = height - pad.top - pad.bottom;

  // Find global x range across all players
  let globalMin = Infinity, globalMax = -Infinity;
  for (const hist of histograms) {
    for (const [score] of hist) {
      if (score < globalMin) globalMin = score;
      if (score > globalMax) globalMax = score;
    }
  }
  if (globalMin === Infinity) { globalMin = 0; globalMax = 10; }
  const xRange = globalMax - globalMin || 1;
  const xScale = v => pad.left + ((v - globalMin) / xRange) * plotW;

  // Find global max count for y scale
  let maxCount = 0;
  for (const hist of histograms) {
    for (const [, count] of hist) {
      if (count > maxCount) maxCount = count;
    }
  }
  if (maxCount === 0) maxCount = 1;
  const yScale = v => pad.top + plotH - (v / maxCount) * plotH;

  let svg = `<svg width="${width}" height="${height}" class="chart-svg">`;

  if (title) {
    svg += `<text x="${width / 2}" y="18" class="chart-title">${title}</text>`;
  }

  // X-axis grid
  const nXTicks = 8;
  for (let i = 0; i <= nXTicks; i++) {
    const val = globalMin + (xRange * i / nXTicks);
    const x = xScale(val);
    svg += `<line x1="${x}" y1="${pad.top}" x2="${x}" y2="${pad.top + plotH}" class="chart-grid"/>`;
    svg += `<text x="${x}" y="${pad.top + plotH + 16}" class="chart-axis-label" text-anchor="middle">${Math.round(val)}</text>`;
  }

  // Y-axis grid
  const nYTicks = 4;
  for (let i = 0; i <= nYTicks; i++) {
    const val = (maxCount * i / nYTicks);
    const y = yScale(val);
    svg += `<line x1="${pad.left}" y1="${y}" x2="${pad.left + plotW}" y2="${y}" class="chart-grid"/>`;
    svg += `<text x="${pad.left - 6}" y="${y + 4}" class="chart-axis-label" text-anchor="end">${Math.round(val)}</text>`;
  }

  // X-axis label
  svg += `<text x="${pad.left + plotW / 2}" y="${height - 8}" class="chart-axis-label" text-anchor="middle">Total Score (4 rounds)</text>`;

  // Draw each player's distribution as a smooth filled area
  for (let p = 0; p < histograms.length; p++) {
    const hist = histograms[p];
    if (hist.length < 2) continue;

    const color = CHART_COLORS[p % CHART_COLORS.length];
    const points = hist.map(([score, count]) => ({ x: xScale(score), y: yScale(count) }));
    const baseline = yScale(0);

    // Build smooth curve using cubic bezier segments
    let curvePath = '';
    for (let i = 0; i < points.length; i++) {
      if (i === 0) {
        curvePath += `M ${points[i].x} ${points[i].y}`;
      } else {
        const p0 = points[i - 1];
        const p1 = points[i];
        const cpx = (p0.x + p1.x) / 2;
        curvePath += ` C ${cpx} ${p0.y}, ${cpx} ${p1.y}, ${p1.x} ${p1.y}`;
      }
    }

    // Filled area
    const fillPath = `M ${points[0].x} ${baseline} L ${points[0].x} ${points[0].y}`
      + curvePath.slice(curvePath.indexOf('C') - 1)
      + ` L ${points[points.length - 1].x} ${baseline} Z`;
    svg += `<path d="${fillPath}" fill="${color}" opacity="0.15" stroke="none"/>`;

    // Stroke
    svg += `<path d="${curvePath}" fill="none" stroke="${color}" stroke-width="2.5" opacity="0.85"/>`;
  }

  // Legend
  const legendY = pad.top + 14;
  for (let p = 0; p < labels.length; p++) {
    const lx = pad.left + 10 + p * 100;
    const color = CHART_COLORS[p % CHART_COLORS.length];
    svg += `<rect x="${lx}" y="${legendY - 8}" width="12" height="12" fill="${color}" opacity="0.7" rx="2"/>`;
    svg += `<text x="${lx + 16}" y="${legendY + 2}" class="chart-legend-label">${labels[p]}</text>`;
  }

  svg += '</svg>';
  return svg;
}
