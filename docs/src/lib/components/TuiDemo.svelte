<script>
  import { onMount } from "svelte";
  import cargoToml from "../../../../Cargo.toml?raw";

  /**
   * A recreation of the dtop container list, drawn as a character grid with
   * the same layout rules as the binary: column widths, header bar fitting,
   * byte formatting and colors. Stats are animated with the same exponential
   * moving average the real stats pipeline uses (alpha = 0.3, 500ms tick).
   *
   * Hand-ported, so keep it in sync when these change:
   * - `src/ui/container_list.rs`: `column_constraints`, `create_header_row`,
   *   `create_container_row`, `get_percentage_style`, `list_block`
   * - `src/ui/system_header.rs`: `HeaderText`, `render_system_header`
   * - `src/ui/formatters.rs`: `write_byte_value`
   * - `src/ui/icons.rs`, and default columns in `src/core/types.rs`
   */
  const version = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1] ?? "";

  // Terminal size in cells. The grid is scaled so this many columns fill the panel.
  const COLS = 160;
  // The list block's proportional(1) padding: 2 cells left and right
  const INNER = COLS - 4;
  const BAR = 20;
  const ALPHA = 0.3;
  const TICK_MS = 500;

  const KB = 1024;
  const MB = KB * 1024;
  const GB = MB * 1024;

  // ANSI colors as the terminal would draw them
  const GREEN = "text-(--c-ok)";
  const YELLOW = "text-(--c-warn)";
  const RED = "text-(--c-crit)";
  const CYAN_BOLD = "font-semibold text-(--c-cyan)";

  /** Docker hosts: capacity from `docker info`, plus how many containers are stopped. */
  const hosts = {
    local: { cores: 10, mem: 7.7 * GB, stopped: 1 },
    "ops@db-eu": { cores: 8, mem: 32 * GB, stopped: 0 },
    "10.0.4.12:2375": { cores: 16, mem: 64 * GB, stopped: 2 },
  };

  /**
   * Machines that get a System line: the one dtop runs on, and SSH hosts (read
   * from their /proc). TCP hosts have no System line. CPU and network are
   * the load outside the containers, added to the containers' own.
   */
  let machines = $state([
    { host: "local", name: "workstation", cores: 10, mem: 32 * GB, memOther: 13.2 * GB, cpu: 14, target: 14, tx: 1.9 * MB, rx: 4.6 * MB, swap: "  Swap 1.2G/4G", load: [2.84, 2.61, 2.40], up: "  Up 8d 9h" },
    { host: "ops@db-eu", name: "ops@db-eu", cores: 8, mem: 32 * GB, memOther: 2.1 * GB, cpu: 6, target: 6, tx: 0.2 * MB, rx: 0.4 * MB, swap: "", load: [1.42, 1.37, 1.29], up: "  Up 41d 2h" },
  ]);

  // Sorted the way dtop sorts by default: Created, newest first
  const seed = [
    ["a1f3c9e27b04", "unhealthy", "worker-etl", "10.0.4.12:2375", 88, 1.6 * GB, 2 * GB, 277 * KB, 999 * KB, 4 * 3600],
    ["7d2e0b91c5aa", "running", "api-gateway", "10.0.4.12:2375", 52, 380 * MB, 1 * GB, 7.8 * MB, 5.8 * MB, 6 * 86400],
    ["e4b8a17f3d62", "running", "nginx-ingress", "10.0.4.12:2375", 18, 96 * MB, 512 * MB, 12 * MB, 9.1 * MB, 6 * 86400],
    ["3c91f0d4a8e7", "running", "postgres-main", "ops@db-eu", 74, 5.2 * GB, 8 * GB, 98 * KB, 5.8 * MB, 12 * 86400],
    ["b06d5e2f9c13", "healthy", "redis-cache", "ops@db-eu", 31, 410 * MB, 2 * GB, 1.1 * MB, 209 * KB, 12 * 86400],
    ["58fa2c7e01bd", "healthy", "grafana", "local", 11, 212 * MB, 7.7 * GB, 705 * KB, 168 * KB, 21 * 86400],
    ["92e7b4d0f6c8", "running", "prometheus", "local", 24, 1.1 * GB, 7.7 * GB, 64 * KB, 2.2 * MB, 21 * 86400],
  ];

  let rows = $state(
    seed.map(([id, status, name, host, cpu, mem, limit, tx, rx, age]) => ({
      id, status, name, host, cpu, mem, limit, tx, rx, age,
      targetCpu: cpu,
      targetMem: mem,
    })),
  );

  // The row the cursor is on, highlighted like ratatui's row_highlight_style
  const SELECTED = 1;

  // `src/ui/icons.rs` (unicode set): health wins over state when present
  const icons = {
    running: ["▶", GREEN],
    healthy: ["✓", GREEN],
    unhealthy: ["✖", RED],
  };

  /** `get_percentage_style` */
  function tone(value) {
    if (value > 80) return RED;
    if (value > 50) return YELLOW;
    return GREEN;
  }

  /** `write_byte_value`, shared by `write_bytes` and `write_bytes_per_sec`. */
  function bytes(value, [gbPrec, mbPrec, kbPrec, bPrec], unit = "") {
    if (value >= GB) {
      const gb = value / GB;
      const prec = gbPrec === 0 && Math.abs(gb - Math.round(gb)) >= 0.05 ? 1 : gbPrec;
      return `${gb.toFixed(prec)}G${unit}`;
    }
    if (value >= MB) return `${(value / MB).toFixed(mbPrec)}M${unit}`;
    if (value >= KB) return `${(value / KB).toFixed(kbPrec)}K${unit}`;
    return `${value.toFixed(bPrec)}B${unit && unit.slice(1)}`;
  }
  const size = (value) => bytes(value, [0, 0, 0, 0]);
  const rate = (value) => bytes(value, [2, 2, 1, 0], "B/s");

  const pct = (value) => `${value.toFixed(1).padStart(5)}%`;

  /** The `timeago` crate's formatting, to one unit. */
  function ago(secs) {
    const units = [["week", 604800], ["day", 86400], ["hour", 3600], ["minute", 60]];
    for (const [unit, length] of units) {
      const n = Math.floor(secs / length);
      if (n >= 1) return `${n} ${unit}${n === 1 ? "" : "s"} ago`;
    }
    return "now";
  }

  /** Clip or pad `text` to exactly `width` cells (all text here is one cell per char). */
  const fit = (text, width) => [...text].slice(0, width).join("").padEnd(width);

  const seg = (t, c = "") => ({ t, c });

  function bar(percent, width, cls) {
    const filled = Math.round((Math.max(0, Math.min(100, percent)) / 100) * width);
    return seg("█".repeat(filled) + "░".repeat(width - filled), cls);
  }

  // ---- Container table (`column_constraints`, spacing 1, Name takes the rest)

  const columns = [
    ["ID", 12],
    ["", 1],
    ["Name", 0],
    ["Host", 20],
    ["CPU %", 28],
    ["Memory %", 33],
    ["Net TX", 12],
    ["Net RX", 12],
    ["Created ▼", 15],
  ];
  const fixed = columns.reduce((sum, [, width]) => sum + width, 0) + columns.length - 1;
  const widths = columns.map(([, width]) => width || INNER - fixed);

  /** One table row as segments, each cell clipped/padded to its column. */
  function tableRow(cells) {
    const out = [];
    cells.forEach((cell, i) => {
      const width = widths[i];
      const parts = Array.isArray(cell) ? cell : [seg(cell)];
      let used = 0;
      for (const part of parts) {
        const room = width - used;
        if (room <= 0) break;
        const text = [...part.t].slice(0, room).join("");
        out.push(seg(text, part.c));
        used += [...text].length;
      }
      out.push(seg(" ".repeat(width - used + (i < cells.length - 1 ? 1 : 0))));
    });
    return out;
  }

  let containerLines = $derived(
    rows.map((row) => {
      const memPct = (row.mem / row.limit) * 100;
      const [icon, iconCls] = icons[row.status];
      return tableRow([
        row.id,
        [seg(icon, iconCls)],
        row.name,
        row.host,
        [bar(row.cpu, BAR, tone(row.cpu)), seg(` ${pct(row.cpu)}`, tone(row.cpu))],
        [bar(memPct, BAR, tone(memPct)), seg(` ${size(row.mem)}/${size(row.limit)}`, tone(memPct))],
        rate(row.tx),
        rate(row.rx),
        ago(row.age),
      ]);
    }),
  );

  // ---- System stats header (`HeaderText` and `render_system_header`)

  const RATE_WIDTH = 10;
  const MEMORY_TEXT_WIDTH = 11;
  const MIN_BAR_WIDTH = 8;
  const NETWORK_GAP = " ".repeat(5 + RATE_WIDTH + 4 + RATE_WIDTH);

  function headerText(kind, label, cpu, cores, used, total) {
    return {
      kind,
      label,
      cpuPct: cpu,
      cpu: pct(cpu),
      cores: ` of ${String(cores).padStart(2)} cores`,
      memPct: (used / total) * 100,
      mem: `${size(used)}/${size(total)}`.padEnd(MEMORY_TEXT_WIDTH),
      net: null,
      swap: "",
      load: "",
      up: "",
      running: "",
    };
  }

  let headerTexts = $derived.by(() => {
    const docker = (host) => {
      const running = rows.filter((row) => row.host === host);
      return {
        running,
        // Container CPU is per core, so the sum is divided by the core count
        cpu: running.reduce((sum, row) => sum + row.cpu, 0) / hosts[host].cores,
        mem: running.reduce((sum, row) => sum + row.mem, 0),
        tx: running.reduce((sum, row) => sum + row.tx, 0),
        rx: running.reduce((sum, row) => sum + row.rx, 0),
      };
    };

    const texts = [];
    for (const host of Object.keys(hosts)) {
      const machine = machines.find((m) => m.host === host);
      const totals = docker(host);
      if (machine) {
        const text = headerText(
          "System",
          machine.name,
          Math.min(100, totals.cpu + machine.cpu),
          machine.cores,
          Math.min(machine.mem, machine.memOther + totals.mem),
          machine.mem,
        );
        text.net = [
          rate(machine.tx + totals.tx).padEnd(RATE_WIDTH),
          rate(machine.rx + totals.rx).padEnd(RATE_WIDTH),
        ];
        text.swap = machine.swap;
        text.load = `  Load ${machine.load.map((n) => n.toFixed(2)).join(" ")}`;
        text.up = machine.up;
        texts.push(text);
      }
      const info = hosts[host];
      const text = headerText("Docker", host, totals.cpu, info.cores, totals.mem, info.mem);
      const total = totals.running.length + info.stopped;
      text.running = `  ${totals.running.length}/${total} running`;
      texts.push(text);
    }
    return texts;
  });

  // Detail levels, shed from the top down when space runs short
  const detail = {
    network: (level) => level >= 1,
    cores: (level) => level >= 2,
    swap: (level) => level >= 3,
    load: (level) => level >= 4,
    up: (level) => level >= 5,
  };

  let headerLines = $derived.by(() => {
    const labelWidth = Math.min(24, Math.max(...headerTexts.map((t) => t.label.length)));
    const label = (text) => `  ${fit(text.label, labelWidth)}  `;

    const textWidth = (text, level) =>
      text.kind.length +
      label(text).length +
      "CPU ".length +
      text.cpu.length +
      "  MEM ".length +
      text.mem.length +
      text.running.length +
      (detail.cores(level) ? text.cores.length : 0) +
      (detail.network(level) ? NETWORK_GAP.length : 0) +
      (detail.swap(level) ? text.swap.length : 0) +
      (detail.load(level) ? text.load.length : 0) +
      (detail.up(level) ? text.up.length : 0);

    let chosen = null;
    for (let level = 5; level >= 0; level--) {
      const needed = Math.max(...headerTexts.map((t) => textWidth(t, level)));
      const barWidth = Math.max(0, Math.floor(Math.max(0, INNER - needed) / 2) - 1);
      if (level >= 2 && barWidth >= MIN_BAR_WIDTH) {
        chosen = [level, barWidth];
        break;
      }
      if (!chosen && needed <= INNER) chosen = [level, 0];
    }
    const [level, barWidth] = chosen ?? [0, 0];

    return headerTexts.map((text) => {
      const cpuCls = tone(text.cpuPct);
      const memCls = tone(text.memPct);
      const out = [seg(text.kind, CYAN_BOLD), seg(label(text)), seg("CPU ", CYAN_BOLD)];
      if (barWidth) out.push(bar(text.cpuPct, barWidth, cpuCls), seg(" "));
      out.push(seg(text.cpu, cpuCls));
      if (detail.cores(level)) out.push(seg(text.cores));
      out.push(seg("  MEM ", CYAN_BOLD));
      if (barWidth) out.push(bar(text.memPct, barWidth, memCls), seg(" "));
      out.push(seg(text.mem, memCls));
      if (detail.network(level) && text.net) {
        out.push(seg("  TX ", CYAN_BOLD), seg(text.net[0]), seg(" RX ", CYAN_BOLD), seg(text.net[1]));
      } else if (detail.network(level)) {
        out.push(seg(NETWORK_GAP));
      }
      for (const [shown, value] of [
        [detail.swap(level), text.swap],
        [detail.load(level), text.load],
        [detail.up(level), text.up],
        [true, text.running],
      ]) {
        if (shown && value) out.push(seg(value));
      }
      return out;
    });
  });

  const headerRow = tableRow(columns.map(([title]) => title));

  let title = $derived(
    `dtop v${version} - ${rows.length} containers ('?' for help, 'q' to quit)`,
  );

  onMount(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;

    const timer = setInterval(() => {
      // Same tick as the rows, so the header and table update in one frame
      for (const machine of machines) {
        if (Math.random() < 0.25) {
          machine.target = Math.max(2, Math.min(30, machine.target + (Math.random() - 0.5) * 10));
        }
        machine.cpu += (machine.target - machine.cpu) * ALPHA;
        machine.tx = Math.max(0.1 * MB, machine.tx + (Math.random() - 0.5) * 0.4 * MB);
        machine.rx = Math.max(0.1 * MB, machine.rx + (Math.random() - 0.5) * 0.4 * MB);
      }

      rows = rows.map((row) => {
        let { targetCpu, targetMem } = row;
        if (Math.random() < 0.25) {
          targetCpu = Math.max(2, Math.min(96, targetCpu + (Math.random() - 0.5) * 30));
          targetMem = Math.max(
            row.limit * 0.02,
            Math.min(row.limit * 0.93, targetMem + (Math.random() - 0.5) * 0.1 * row.limit),
          );
        }
        const jitter = (value) => Math.max(0, value * (1 + (Math.random() - 0.5) * 0.3));

        return {
          ...row,
          targetCpu,
          targetMem,
          cpu: row.cpu + (targetCpu - row.cpu) * ALPHA,
          mem: row.mem + (targetMem - row.mem) * ALPHA,
          tx: row.tx + (jitter(row.tx) - row.tx) * ALPHA,
          rx: row.rx + (jitter(row.rx) - row.rx) * ALPHA,
        };
      });
    }, TICK_MS);

    return () => clearInterval(timer);
  });
</script>

{#snippet line(segments)}
  {#each segments as s}{#if s.c}<span class={s.c}>{s.t}</span>{:else}{s.t}{/if}{/each}
{/snippet}

<div class="panel overflow-hidden" data-tui-demo>
  <div class="flex items-center gap-1.5 border-b border-(--c-line) bg-(--c-surface-2) px-3.5 py-2.5">
    {#each [0, 1, 2] as _}
      <span class="size-2.5 rounded-full bg-(--c-line-2)"></span>
    {/each}
  </div>

  <div class="overflow-x-auto px-2 py-2 [container-type:inline-size]">
    <pre
      class="w-max min-w-full font-mono text-[max(7px,calc(100cqw/(160*0.6)))] leading-[1.3] text-(--c-text)"
      role="img"
      aria-label="dtop showing live CPU, memory and network stats for containers across three Docker hosts">{title}

{#each headerLines as segments}  {@render line(segments)}
{/each}
<span class={CYAN_BOLD}>  {@render line(headerRow)}</span>

{#each containerLines as segments, i}{#if i === SELECTED}  <span
          class="bg-(--c-line-2) font-semibold">{@render line(segments)}</span
        >{:else}  {@render line(segments)}{/if}
{/each}</pre>
  </div>
</div>
