<script>
  import { onMount } from "svelte";

  /**
   * A recreation of the dtop container table, animated with the same
   * exponential moving average the real stats pipeline uses (alpha = 0.3,
   * 500ms tick) so the motion matches what the binary actually draws.
   */
  const GAUGE_WIDTH = 12;
  const ALPHA = 0.3;
  const TICK_MS = 500;

  const states = {
    running: { class: "text-(--c-ok)", icon: "●", label: "running" },
    unhealthy: { class: "text-(--c-warn)", icon: "▲", label: "unhealthy" },
    exited: { class: "text-(--c-text-dim)", icon: "○", label: "exited" },
  };

  const seed = [
    ["running", "postgres-primary", "ops@db-eu", 74, 61, 1.2, 3.8, 41, "12d"],
    ["running", "api-gateway", "deploy@edge-01", 52, 38, 8.4, 6.1, 27, "6d"],
    ["running", "redis-cache", "ops@db-eu", 31, 22, 0.9, 1.4, 12, "12d"],
    ["running", "nginx-ingress", "deploy@edge-01", 18, 14, 12.6, 9.3, 9, "6d"],
    ["unhealthy", "worker-etl", "ops@db-us", 88, 79, 0.4, 0.2, 33, "4h"],
    ["running", "grafana", "local", 11, 27, 0.3, 0.6, 18, "21d"],
    ["running", "prometheus", "local", 24, 44, 0.8, 2.2, 16, "21d"],
    ["exited", "legacy-cron", "ops@db-us", 0, 0, 0, 0, 0, "—"],
  ];

  const hosts = [
    { name: "all", state: "running" },
    { name: "local", state: "running" },
    { name: "deploy@edge-01", state: "running" },
    { name: "ops@db-eu", state: "running" },
    { name: "ops@db-us", state: "unhealthy" },
  ];

  let rows = $state(
    seed.map(([state, name, host, cpu, mem, tx, rx, pids, uptime]) => ({
      state,
      name,
      host,
      cpu,
      mem,
      tx,
      rx,
      pids,
      uptime,
      targetCpu: cpu,
      targetMem: mem,
    })),
  );

  /**
   * The stats header above the table. Docker lines are summed from the rows
   * below (container CPU is per core, so it's divided by the host's cores);
   * System lines add the rest of the machine's load on top. Like the real
   * header, the local machine comes first and each host's lines are grouped.
   */
  const CONTAINER_MEM_GB = 4;
  const machines = [
    { host: "local", label: "workstation", cores: 10, memGb: 32 },
    { host: "ops@db-eu", label: "ops@db-eu", cores: 8, memGb: 32 },
  ];
  // Load outside the containers (desktop apps, the OS), animated like the rows
  let machineLoad = $state(
    Object.fromEntries([
      ["local", { cpu: 14, target: 14, memGb: 13.2, tx: 3.1, rx: 6.4 }],
      ["ops@db-eu", { cpu: 9, target: 9, memGb: 6.6, tx: 0.4, rx: 0.3 }],
    ]),
  );

  let headerLines = $derived(
    machines.flatMap(({ host, label, cores, memGb }) => {
      const hostRows = rows.filter((row) => row.host === host);
      const running = hostRows.filter((row) => row.state !== "exited");
      const sum = (key) => running.reduce((total, row) => total + row[key], 0);
      const docker = {
        cpu: sum("cpu") / cores,
        mem: running.reduce((total, row) => total + (row.mem / 100) * CONTAINER_MEM_GB, 0),
        tx: sum("tx"),
        rx: sum("rx"),
      };
      const load = machineLoad[host];
      return [
        {
          kind: "System",
          label,
          cores,
          cpu: Math.min(100, docker.cpu + load.cpu),
          memUsed: docker.mem + load.memGb,
          memGb,
          tx: docker.tx + load.tx,
          rx: docker.rx + load.rx,
          extra: "",
        },
        {
          kind: "Docker",
          label: host,
          cores,
          cpu: docker.cpu,
          memUsed: docker.mem,
          memGb,
          tx: docker.tx,
          rx: docker.rx,
          extra: `${running.length}/${hostRows.length} running`,
        },
      ];
    }),
  );

  function tone(value) {
    if (value > 80) return "text-(--c-crit)";
    if (value > 50) return "text-(--c-warn)";
    return "text-(--c-ok)";
  }

  function filled(value) {
    return "█".repeat(
      Math.max(0, Math.min(GAUGE_WIDTH, Math.round((value / 100) * GAUGE_WIDTH))),
    );
  }

  function empty(value) {
    return "░".repeat(
      GAUGE_WIDTH -
        Math.max(0, Math.min(GAUGE_WIDTH, Math.round((value / 100) * GAUGE_WIDTH))),
    );
  }

  function rate(value) {
    return value < 1
      ? `${(value * 1000).toFixed(0)} KB/s`
      : `${value.toFixed(1)} MB/s`;
  }

  onMount(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;

    const timer = setInterval(() => {
      rows = rows.map((row) => {
        if (row.state === "exited") return row;

        let { targetCpu, targetMem } = row;
        if (Math.random() < 0.25) {
          targetCpu = Math.max(2, Math.min(96, targetCpu + (Math.random() - 0.5) * 30));
          targetMem = Math.max(2, Math.min(93, targetMem + (Math.random() - 0.5) * 10));
        }

        return {
          ...row,
          targetCpu,
          targetMem,
          cpu: row.cpu + (targetCpu - row.cpu) * ALPHA,
          mem: row.mem + (targetMem - row.mem) * ALPHA,
          tx: Math.max(0, row.tx + (Math.random() - 0.5) * 1.4),
          rx: Math.max(0, row.rx + (Math.random() - 0.5) * 1.4),
        };
      });
    }, TICK_MS);

    const loadTimer = setInterval(() => {
      for (const load of Object.values(machineLoad)) {
        if (Math.random() < 0.25) {
          load.target = Math.max(2, Math.min(40, load.target + (Math.random() - 0.5) * 12));
        }
        load.cpu += (load.target - load.cpu) * ALPHA;
        load.tx = Math.max(0.1, load.tx + (Math.random() - 0.5) * 0.6);
        load.rx = Math.max(0.1, load.rx + (Math.random() - 0.5) * 0.6);
      }
    }, TICK_MS);

    return () => {
      clearInterval(timer);
      clearInterval(loadTimer);
    };
  });
</script>

<div class="panel overflow-hidden">
  <div
    class="flex items-center gap-3 border-b border-(--c-line) bg-(--c-surface-2) px-3.5 py-2 font-mono text-xs text-(--c-text-dim)"
  >
    <span class="font-medium text-(--c-text)">dtop</span>
    <span class="flex-1"></span>
    <span class="hidden sm:inline">sort cpu &darr;</span>
    <span class="hidden sm:inline">4 hosts</span>
    <span>{rows.length} containers</span>
  </div>

  <div class="overflow-x-auto">
    <div class="flex min-w-160 border-b border-(--c-line) font-mono text-xs">
      {#each hosts as host, i}
        <span
          class="flex items-center gap-1.5 px-3.5 py-1.5"
          class:text-(--c-text)={i === 0}
          class:text-(--c-text-dim)={i !== 0}
          class:shadow-[inset_0_-1px_0_var(--c-accent)]={i === 0}
        >
          <span class={states[host.state].class}>●</span>
          {host.name}
        </span>
      {/each}
    </div>

    <table
      class="w-full min-w-160 border-collapse border-b border-(--c-line) font-mono text-[clamp(0.6rem,0.95vw,0.78rem)] leading-[1.7] tabular-nums"
    >
      <tbody class="[&>tr:first-child>td]:pt-1.5 [&>tr:last-child>td]:pb-1.5">
        {#each headerLines as line}
          <tr>
            <td class="py-0 pr-2.5 pl-3.5 font-medium whitespace-nowrap text-(--c-accent)">{line.kind}</td>
            <td class="px-2.5 py-0 whitespace-nowrap text-(--c-text)">{line.label}</td>
            <td class="px-2.5 py-0 whitespace-nowrap">
              <span class="text-(--c-text-dim)">CPU</span>
              <span class="tracking-[-0.05em] {tone(line.cpu)}">{filled(line.cpu)}</span
              ><span class="tracking-[-0.05em] text-(--c-track)">{empty(line.cpu)}</span>
              <span class="whitespace-pre {tone(line.cpu)}"
                >{String(Math.round(line.cpu)).padStart(3, " ")}%</span
              >
              <span class="whitespace-pre text-(--c-text-dim)"
                >of {String(line.cores).padStart(2, " ")} cores</span
              >
            </td>
            <td class="px-2.5 py-0 whitespace-nowrap">
              <span class="text-(--c-text-dim)">MEM</span>
              <span class="tracking-[-0.05em] {tone((line.memUsed / line.memGb) * 100)}"
                >{filled((line.memUsed / line.memGb) * 100)}</span
              ><span class="tracking-[-0.05em] text-(--c-track)"
                >{empty((line.memUsed / line.memGb) * 100)}</span
              >
              <span class={tone((line.memUsed / line.memGb) * 100)}
                >{line.memUsed.toFixed(1)}G/{line.memGb}G</span
              >
            </td>
            <td class="px-2.5 py-0 whitespace-nowrap text-(--c-text-muted)">
              <span class="text-(--c-text-dim)">TX</span> {rate(line.tx)}
            </td>
            <td class="px-2.5 py-0 whitespace-nowrap text-(--c-text-muted)">
              <span class="text-(--c-text-dim)">RX</span> {rate(line.rx)}
            </td>
            <td class="w-full px-2.5 py-0 whitespace-nowrap text-(--c-text-muted)">{line.extra}</td>
          </tr>
        {/each}
      </tbody>
    </table>

    <table
      class="w-full min-w-160 border-collapse font-mono text-[clamp(0.6rem,0.95vw,0.78rem)] leading-[1.85] tabular-nums"
    >
      <thead>
        <tr>
          {#each ["state", "name", "host", "cpu", "mem", "net tx", "net rx", "pids", "up"] as heading}
            <th
              class="border-b border-(--c-line) px-2.5 py-1.5 text-left text-[0.9em] font-medium tracking-[0.08em] whitespace-nowrap text-(--c-text-dim) uppercase"
            >
              {heading}
            </th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#each rows as row, i}
          <tr class={i === 1 ? "bg-(--c-accent-soft)" : ""}>
            <td
              class="px-2.5 py-0.5 whitespace-nowrap {states[row.state].class}"
              class:shadow-[inset_2px_0_0_var(--c-accent)]={i === 1}
            >
              {states[row.state].icon}
              {states[row.state].label}
            </td>
            <td class="px-2.5 py-0.5 whitespace-nowrap text-(--c-text)">{row.name}</td>
            <td class="px-2.5 py-0.5 whitespace-nowrap text-(--c-text-muted)">{row.host}</td>

            {#each [row.cpu, row.mem] as value}
              <td class="px-2.5 py-0.5 whitespace-nowrap">
                <span class="tracking-[-0.05em] {tone(value)}">{filled(value)}</span
                ><span class="tracking-[-0.05em] text-(--c-track)">{empty(value)}</span>
                <span class={tone(value)}>{String(Math.round(value)).padStart(3, " ")}%</span>
              </td>
            {/each}

            <td class="px-2.5 py-0.5 whitespace-nowrap text-(--c-text-muted)">{rate(row.tx)}</td>
            <td class="px-2.5 py-0.5 whitespace-nowrap text-(--c-text-muted)">{rate(row.rx)}</td>
            <td class="px-2.5 py-0.5 whitespace-nowrap text-(--c-text-muted)">{row.pids || "—"}</td>
            <td class="px-2.5 py-0.5 whitespace-nowrap text-(--c-text-muted)">{row.uptime}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  <div
    class="flex flex-wrap gap-x-4 gap-y-1 border-t border-(--c-line) bg-(--c-surface-2) px-3.5 py-2 font-mono text-xs text-(--c-text-dim)"
  >
    {#each [["↑↓", "move"], ["→", "logs"], ["/", "search"], ["s", "sort"], ["c", "columns"], ["a", "all"], ["t", "stats"], ["?", "help"]] as [key, label]}
      <span><span class="font-medium text-(--c-text-muted)">{key}</span> {label}</span>
    {/each}
  </div>
</div>
