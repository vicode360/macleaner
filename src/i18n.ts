import type { AppLanguage, RiskLevel } from "./types";

export interface CopyBundle {
  htmlLang: string;
  locale: string;
  tabs: {
    resumo: string;
    itens: string;
    dev: string;
    ajustes: string;
    navLabel: string;
  };
  confirm: {
    permanentWarning: string;
  };
  header: {
    updateIdle: string;
    checkingUpdate: string;
    installingUpdate: string;
    downloadingPct: (pct: number) => string;
    installVersion: (ver: string) => string;
    checkUpdateTitle: string;
    rescanAria: string;
  };
  disk: {
    localFallback: string;
    readingVolume: string;
    healthy: string;
    warn: string;
    critical: string;
    freeOf: (free: string, total: string) => string;
    inUse: string;
    mapped: string;
  };
  summary: {
    smartCleanup: string;
    readyToScan: string;
    foldersAnalyzed: (count: string) => string;
    idleHint: string;
    scanDisk: string;
    rescan: string;
    scanning: string;
    startingScan: string;
    scanningPath: (path: string) => string;
    cleanSafeCount: (count: number) => string;
    safeLabel: (size: string) => string;
    largestCulprits: string;
    viewAll: string;
  };
  items: {
    emptyTitle: string;
    emptyDesc: string;
    filterAll: string;
    cleanSafe: string;
    filterEmptyTitle: string;
    filterEmptyDesc: string;
    filesCount: (count: string) => string;
    cleanableNote: (size: string) => string;
    subfoldersCount: (count: number) => string;
    hide: string;
    revealInFinder: string;
    ignoreAlways: string;
    trash: string;
    confirmTrashItem: (name: string, size: string) => string;
    confirmTrashSafe: (count: number, size: string) => string;
    cancel: string;
    moveToTrash: string;
    moving: string;
  };
  snapshots: {
    title: string;
    refreshAria: string;
    loading: string;
    unsupported: string;
    empty: string;
    summaryLine: (count: number) => string;
    hint: string;
    show: string;
    hide: string;
    clearAll: string;
    remove: string;
    removing: string;
    confirmSingle: (date: string) => string;
    confirmAll: (count: number) => string;
    confirmAction: string;
  };
  risk: Record<RiskLevel, string>;
  settings: {
    loading: string;
    languageLabel: string;
    languageHint: string;
    systemSectionLabel: string;
    autostartLabel: string;
    autostartHint: string;
    lowDiskAlertLabel: string;
    lowDiskAlertHint: string;
    allowlistLabel: string;
    allowlistEmpty: string;
    allowlistHint: string;
    allowlistRestore: string;
    minFolderSize: string;
    minSizeAria: string;
    aiEngineLabel: string;
    providerOllama: string;
    providerExternal: string;
    providerDisabled: string;
    ollamaEndpoint: string;
    localModel: string;
    autoModel: string;
    ollamaHint: string;
    externalEndpoint: string;
    model: string;
    apiKey: string;
    disabledHint: string;
    detectModels: string;
    detecting: string;
    saveSettings: string;
    saving: string;
  };
  toasts: {
    diskError: (err: string) => string;
    scanComplete: (size: string) => string;
    scanFailed: (err: string) => string;
    trashNoneMoved: (reason: string) => string;
    trashPartial: (moved: number, failed: number) => string;
    trashMoved: (size: string) => string;
    trashError: (err: string) => string;
    revealError: (err: string) => string;
    pathIgnored: (name: string) => string;
    pathUnignored: (name: string) => string;
    ignoreError: (err: string) => string;
    autostartError: (err: string) => string;
    snapshotsLoadError: (err: string) => string;
    snapshotsDeleted: (count: number) => string;
    snapshotsPartial: (deleted: number, failed: number) => string;
    snapshotsCancelled: string;
    snapshotsError: (err: string) => string;
    updateError: (err: string) => string;
    updateInstalling: string;
    updateRestarting: string;
    updateAvailable: (latest: string, current: string) => string;
    updateLatest: (version: string) => string;
    settingsSaved: string;
    modelsDetected: (list: string) => string;
    noModelsFound: string;
  };
  days: {
    today: string;
    oneDay: string;
    nDays: (n: string) => string;
  };
  dev: {
    subtabs: {
      projects: string;
      xcode: string;
      docker: string;
      tools: string;
    };
    refreshAria: string;
    loading: string;
    loadError: (err: string) => string;
    cleanAction: string;
    cleaningAction: string;
    deleteAction: string;
    deletingAction: string;
    runAction: string;
    runningAction: string;
    projects: {
      rootsTitle: string;
      addFolder: string;
      scan: string;
      scanning: string;
      removeRootAria: (name: string) => string;
      noRootsHint: string;
      noRootsTitle: string;
      noRootsDesc: string;
      filterAll: string;
      filter30: string;
      filter90: string;
      cleanOlder: (days: number) => string;
      emptyTitle: string;
      emptyDesc: string;
      filterEmptyTitle: string;
      filterEmptyDesc: string;
      summaryLine: (count: string, size: string) => string;
      inactiveShort: (days: number) => string;
      confirmTrashArtifact: (artifact: string, project: string, size: string) => string;
      confirmTrashBulk: (count: number, size: string, days: number) => string;
      rootAdded: (name: string) => string;
      rootRemoved: (name: string) => string;
      rootError: (err: string) => string;
      scanError: (err: string) => string;
    };
    xcode: {
      notInstalledTitle: string;
      notInstalledDesc: string;
      summaryTitle: string;
      allCleanTitle: string;
      allCleanDesc: string;
      unavailableTitle: string;
      unavailableDesc: string;
      cleanUnavailable: string;
      runtimesTitle: string;
      areasTitle: string;
      categoryLabels: Record<string, string>;
      moreCount: (n: number) => string;
      confirmUnavailable: (count: number, size: string) => string;
      confirmRuntime: (name: string, size: string) => string;
      confirmTrashArea: (name: string, size: string) => string;
      unavailableCleaned: (count: number, size: string) => string;
      runtimeDeleted: (name: string, size: string) => string;
      actionError: (err: string) => string;
    };
    docker: {
      notInstalledTitle: string;
      notInstalledDesc: string;
      stoppedTitle: string;
      stoppedDesc: (engine: string) => string;
      retry: string;
      footprintLabel: string;
      reclaimableLabel: string;
      resourcesTitle: string;
      activeOfTotal: (active: string, total: string) => string;
      reclaimablePct: (size: string, pct: number) => string;
      clean: string;
      cleanStatus: string;
      readOnlyBadge: string;
      volumesSafetyNote: string;
      kindLabels: Record<string, string>;
      confirmPrune: (kindLabel: string, size: string) => string;
      pruneSuccess: (size: string) => string;
      pruneError: (err: string) => string;
    };
    tools: {
      brewTitle: string;
      dryRunBadge: string;
      brewNotInstalled: string;
      brewClean: string;
      brewOutdatedSummary: (count: string, size: string) => string;
      showPreview: (count: number) => string;
      hidePreview: string;
      runCleanup: string;
      autoremoveTitle: (count: number) => string;
      runAutoremove: string;
      cachesTitle: string;
      cachesEmpty: string;
      confirmBrewCleanup: (size: string) => string;
      confirmBrewAutoremove: (count: number) => string;
      confirmTrashCache: (name: string, size: string) => string;
      brewSuccess: (action: string, size: string) => string;
      brewError: (err: string) => string;
      cacheTrashed: (name: string, size: string) => string;
    };
  };
}

const BUNDLES: Record<AppLanguage, CopyBundle> = {
  "pt-br": {
    htmlLang: "pt-BR",
    locale: "pt-BR",
    tabs: {
      resumo: "Resumo",
      itens: "Itens",
      dev: "Dev",
      ajustes: "Ajustes",
      navLabel: "Navegação principal",
    },
    confirm: {
      permanentWarning: "Ação permanente — não vai para a Lixeira",
    },
    header: {
      updateIdle: "Update",
      checkingUpdate: "Checando…",
      installingUpdate: "Atualizando…",
      downloadingPct: (pct) => `Baixando ${pct}%…`,
      installVersion: (ver) => `Instalar v${ver}`,
      checkUpdateTitle: "Verificar atualizações",
      rescanAria: "Escanear disco novamente",
    },
    disk: {
      localFallback: "Disco Local",
      readingVolume: "Lendo volume…",
      healthy: "Saudável",
      warn: "Atenção",
      critical: "Crítico",
      freeOf: (free, total) => `${free} livres de ${total}`,
      inUse: "Em uso",
      mapped: "Mapeado",
    },
    summary: {
      smartCleanup: "Limpeza Inteligente",
      readyToScan: "Pronto para escanear",
      foldersAnalyzed: (count) => `${count} pastas pesadas analisadas`,
      idleHint: "Varre caches, Xcode, navegadores e subpastas com envio seguro à Lixeira.",
      scanDisk: "Escanear Disco",
      rescan: "Escanear Novamente",
      scanning: "Escaneando…",
      startingScan: "Iniciando varredura…",
      scanningPath: (path) => `Escaneando ${path}…`,
      cleanSafeCount: (count) => `Limpar Seguros (${count})`,
      safeLabel: (size) => `${size} seguros`,
      largestCulprits: "Maiores Vilões",
      viewAll: "Ver todos",
    },
    items: {
      emptyTitle: "Nenhum scan realizado",
      emptyDesc: "Execute uma varredura para listar as pastas pesadas e subpastas.",
      filterAll: "Todos",
      cleanSafe: "Limpar Seguros",
      filterEmptyTitle: "Nenhum item neste filtro",
      filterEmptyDesc: "Selecione outro nível de risco ou reescaneie.",
      filesCount: (count) => `${count} arq`,
      cleanableNote: (size) => ` · ${size} limpáveis`,
      subfoldersCount: (count) => `Subpastas (${count})`,
      hide: "Ocultar",
      revealInFinder: "Mostrar no Finder",
      ignoreAlways: "Ignorar sempre",
      trash: "Lixeira",
      confirmTrashItem: (name, size) => `Mover "${name}" (${size}) para a Lixeira?`,
      confirmTrashSafe: (count, size) => `Mover ${count} itens seguros (${size}) para a Lixeira?`,
      cancel: "Cancelar",
      moveToTrash: "Mover para Lixeira",
      moving: "Movendo…",
    },
    snapshots: {
      title: "Snapshots APFS",
      refreshAria: "Atualizar snapshots APFS",
      loading: "Verificando snapshots locais do Time Machine…",
      unsupported: "Snapshots APFS locais estão disponíveis apenas no macOS.",
      empty: "Nenhum snapshot local do Time Machine encontrado.",
      summaryLine: (count) =>
        count === 1
          ? "1 snapshot local do Time Machine ocupando Dados do Sistema."
          : `${count} snapshots locais do Time Machine ocupando Dados do Sistema.`,
      hint: "Snapshots locais do Time Machine inflam Dados do Sistema. A remoção é permanente.",
      show: "Ver",
      hide: "Ocultar",
      clearAll: "Limpar todos",
      remove: "Remover",
      removing: "Removendo…",
      confirmSingle: (date) => `Remover permanentemente o snapshot APFS de ${date}?`,
      confirmAll: (count) => `Remover permanentemente ${count} snapshots APFS locais?`,
      confirmAction: "Remover Snapshot",
    },
    risk: {
      baixo: "Baixo",
      medio: "Médio",
      alto: "Alto",
    },
    settings: {
      loading: "Carregando ajustes…",
      languageLabel: "Idioma",
      languageHint: "Define o idioma da interface e das análises geradas pela IA.",
      systemSectionLabel: "Sistema & Barra de Menus",
      autostartLabel: "Iniciar com o macOS",
      autostartHint: "Abre o Macleaner na barra de menus ao iniciar a sessão.",
      lowDiskAlertLabel: "Alertar com pouco espaço (< 15% livre)",
      lowDiskAlertHint: "Envia uma notificação nativa quando o volume ficar abaixo de 15% livres.",
      allowlistLabel: "Pastas Ignoradas (Allowlist)",
      allowlistEmpty: "Nenhuma pasta ignorada. Use \"Ignorar sempre\" em Itens para pular pastas no scan.",
      allowlistHint: "Essas pastas são puladas no scan e na IA. Remova para analisar novamente.",
      allowlistRestore: "Remover",
      minFolderSize: "Tamanho mínimo de pasta",
      minSizeAria: "Tamanho mínimo em megabytes",
      aiEngineLabel: "Motor de IA (executado apenas no scan)",
      providerOllama: "IA Local (Ollama)",
      providerExternal: "API Externa (HTTPS)",
      providerDisabled: "Desativada (Apenas regras)",
      ollamaEndpoint: "Endpoint do Ollama",
      localModel: "Modelo local",
      autoModel: "Auto (detectar modelo instalado)",
      ollamaHint: "Analisa subpastas reais durante o scan e libera a RAM ao terminar.",
      externalEndpoint: "Endpoint OpenAI-compatible (HTTPS)",
      model: "Modelo",
      apiKey: "Chave da API",
      disabledHint: "Pastas fora das regras locais permanecem marcadas como Alto risco.",
      detectModels: "Detectar Modelos",
      detecting: "Detectando…",
      saveSettings: "Salvar Ajustes",
      saving: "Salvando…",
    },
    toasts: {
      diskError: (err) => `Erro ao ler disco: ${err}`,
      scanComplete: (size) => `Scan concluído: ${size} encontrados`,
      scanFailed: (err) => `Falha no scan: ${err}`,
      trashNoneMoved: (reason) => `Não foi possível mover: ${reason}`,
      trashPartial: (moved, failed) => `${moved} movidos para a Lixeira (${failed} falharam)`,
      trashMoved: (size) => `${size} movidos para a Lixeira`,
      trashError: (err) => `Erro ao mover para Lixeira: ${err}`,
      revealError: (err) => `Não foi possível abrir no Finder: ${err}`,
      pathIgnored: (name) => `"${name}" adicionada às pastas ignoradas`,
      pathUnignored: (name) => `"${name}" removida da allowlist — reescaneie para analisar`,
      ignoreError: (err) => `Erro ao atualizar pastas ignoradas: ${err}`,
      autostartError: (err) => `Erro ao configurar início automático: ${err}`,
      snapshotsLoadError: (err) => `Erro ao listar snapshots APFS: ${err}`,
      snapshotsDeleted: (count) =>
        count === 1 ? "1 snapshot APFS removido" : `${count} snapshots APFS removidos`,
      snapshotsPartial: (deleted, failed) =>
        `${deleted} snapshots removidos (${failed} falharam)`,
      snapshotsCancelled: "Remoção de snapshot cancelada",
      snapshotsError: (err) => `Erro ao remover snapshot APFS: ${err}`,
      updateError: (err) => `Erro ao atualizar: ${err}`,
      updateInstalling: "Baixando e instalando atualização…",
      updateRestarting: "Atualização pronta. Reiniciando Macleaner…",
      updateAvailable: (latest, current) => `Nova versão v${latest} disponível (atual: v${current})`,
      updateLatest: (version) => `Você já está na versão mais recente (v${version})`,
      settingsSaved: "Ajustes salvos",
      modelsDetected: (list) => `Modelos detectados: ${list}`,
      noModelsFound: "Nenhum modelo encontrado no Ollama",
    },
    days: {
      today: "hoje",
      oneDay: "há 1 dia",
      nDays: (n) => `há ${n} dias`,
    },
    dev: {
      subtabs: {
        projects: "Projetos",
        xcode: "Xcode",
        docker: "Docker",
        tools: "Tools",
      },
      refreshAria: "Atualizar seção Dev",
      loading: "Inspecionando ambiente…",
      loadError: (err) => `Erro ao inspecionar: ${err}`,
      cleanAction: "Limpar",
      cleaningAction: "Limpando…",
      deleteAction: "Remover",
      deletingAction: "Removendo…",
      runAction: "Executar",
      runningAction: "Executando…",
      projects: {
        rootsTitle: "Pastas Dev",
        addFolder: "Pasta",
        scan: "Escanear",
        scanning: "Escaneando…",
        removeRootAria: (name) => `Remover pasta ${name}`,
        noRootsHint: "Adicione uma pasta de código (ex.: ~/Projects) para encontrar node_modules, target e .venv.",
        noRootsTitle: "Nenhuma pasta configurada",
        noRootsDesc: "Adicione suas pastas de projetos para detectar artefatos de build recuperáveis.",
        filterAll: "Todos",
        filter30: ">30d",
        filter90: ">90d",
        cleanOlder: (days) => `Limpar >${days}d`,
        emptyTitle: "Nenhum artefato encontrado",
        emptyDesc: "Nenhum artefato pesado (node_modules, target, .venv, build) detectado nas pastas monitoradas.",
        filterEmptyTitle: "Nenhum projeto neste filtro",
        filterEmptyDesc: "Tente outro período de inatividade ou reescaneie suas pastas.",
        summaryLine: (count, size) => `${count} projetos · ${size}`,
        inactiveShort: (days) => (days === 0 ? "hoje" : `${days}d`),
        confirmTrashArtifact: (artifact, project, size) =>
          `Mover "${artifact}" de ${project} (${size}) para a Lixeira?`,
        confirmTrashBulk: (count, size, days) =>
          `Mover ${count} artefatos de projetos inativos há >${days}d (${size}) para a Lixeira?`,
        rootAdded: (name) => `Pasta "${name}" adicionada`,
        rootRemoved: (name) => `Pasta "${name}" removida`,
        rootError: (err) => `Erro ao gerenciar pastas: ${err}`,
        scanError: (err) => `Falha ao escanear projetos: ${err}`,
      },
      xcode: {
        notInstalledTitle: "Xcode não detectado",
        notInstalledDesc: "Nenhum dado do Xcode ou simuladores foi encontrado neste Mac.",
        summaryTitle: "Xcode & Simuladores",
        allCleanTitle: "Xcode limpo",
        allCleanDesc: "Nenhum cache pesado, simulador indisponível ou runtime encontrado.",
        unavailableTitle: "Simuladores indisponíveis",
        unavailableDesc: "Dispositivos órfãos de versões antigas do Xcode.",
        cleanUnavailable: "Limpar indisponíveis",
        runtimesTitle: "Runtimes de Simulador",
        areasTitle: "Áreas do Xcode",
        categoryLabels: {
          derived_data: "DerivedData",
          archives: "Arquivos",
          device_support: "DeviceSupport",
          simulator_caches: "Caches",
          previews: "Previews",
        },
        moreCount: (n) => `+${n} mais`,
        confirmUnavailable: (count, size) =>
          `Remover ${count} simuladores indisponíveis (${size}) via simctl?`,
        confirmRuntime: (name, size) =>
          `Remover runtime "${name}" (${size}) via simctl?`,
        confirmTrashArea: (name, size) =>
          `Mover "${name}" (${size}) para a Lixeira?`,
        unavailableCleaned: (count, size) =>
          `${count} simuladores removidos (${size})`,
        runtimeDeleted: (name, size) =>
          `Runtime "${name}" removido (${size})`,
        actionError: (err) => `Erro no Xcode: ${err}`,
      },
      docker: {
        notInstalledTitle: "Docker / OrbStack não detectado",
        notInstalledDesc: "Instale o Docker Desktop ou OrbStack para inspecionar imagens, containers e caches.",
        stoppedTitle: "Daemon parado",
        stoppedDesc: (engine) => `Inicie o ${engine} para inspecionar e limpar recursos de container.`,
        retry: "Tentar novamente",
        footprintLabel: "Uso em disco",
        reclaimableLabel: "Recuperável",
        resourcesTitle: "Recursos",
        activeOfTotal: (active, total) => `${active} ativos de ${total}`,
        reclaimablePct: (size, pct) => `${size} (${pct}%)`,
        clean: "Limpar",
        cleanStatus: "Limpo",
        readOnlyBadge: "Somente leitura",
        volumesSafetyNote: "Volumes podem conter bancos de dados e nunca são limpos automaticamente.",
        kindLabels: {
          "build-cache": "Build Cache",
          images: "Imagens",
          containers: "Containers",
          volumes: "Volumes Locais",
        },
        confirmPrune: (kindLabel, size) =>
          `Executar prune permanente em ${kindLabel} (~${size} recuperáveis)?`,
        pruneSuccess: (size) => `Docker limpo: ${size} liberados`,
        pruneError: (err) => `Erro ao limpar Docker: ${err}`,
      },
      tools: {
        brewTitle: "Homebrew",
        dryRunBadge: "Dry-run",
        brewNotInstalled: "Homebrew não detectado neste Mac.",
        brewClean: "Homebrew já está limpo — nenhum pacote obsoleto encontrado.",
        brewOutdatedSummary: (count, size) => `${count} itens antigos · ~${size} recuperáveis`,
        showPreview: (count) => `Prévia (${count})`,
        hidePreview: "Ocultar prévia",
        runCleanup: "Executar brew cleanup",
        autoremoveTitle: (count) => `${count} dependências órfãs (autoremove)`,
        runAutoremove: "Executar brew autoremove",
        cachesTitle: "Caches de Toolchains",
        cachesEmpty: "Nenhum cache pesado de toolchain detectado.",
        confirmBrewCleanup: (size) =>
          `Executar "brew cleanup" permanentemente (~${size} recuperáveis)?`,
        confirmBrewAutoremove: (count) =>
          `Executar "brew autoremove" para remover ${count} dependências órfãs?`,
        confirmTrashCache: (name, size) =>
          `Mover cache "${name}" (${size}) para a Lixeira?`,
        brewSuccess: (action, size) =>
          `${action} concluído (${size})`,
        brewError: (err) => `Erro no Homebrew: ${err}`,
        cacheTrashed: (name, size) =>
          `"${name}" (${size}) movido para a Lixeira`,
      },
    },
  },
  en: {
    htmlLang: "en",
    locale: "en-US",
    tabs: {
      resumo: "Summary",
      itens: "Items",
      dev: "Dev",
      ajustes: "Settings",
      navLabel: "Main navigation",
    },
    confirm: {
      permanentWarning: "Permanent action — does not go to Trash",
    },
    header: {
      updateIdle: "Update",
      checkingUpdate: "Checking…",
      installingUpdate: "Updating…",
      downloadingPct: (pct) => `Downloading ${pct}%…`,
      installVersion: (ver) => `Install v${ver}`,
      checkUpdateTitle: "Check for updates",
      rescanAria: "Scan disk again",
    },
    disk: {
      localFallback: "Local Disk",
      readingVolume: "Reading volume…",
      healthy: "Healthy",
      warn: "Warning",
      critical: "Critical",
      freeOf: (free, total) => `${free} free of ${total}`,
      inUse: "Used",
      mapped: "Mapped",
    },
    summary: {
      smartCleanup: "Smart Cleanup",
      readyToScan: "Ready to scan",
      foldersAnalyzed: (count) => `${count} heavy folders analyzed`,
      idleHint: "Scans caches, Xcode, browsers, and subfolders with safe Trash removal.",
      scanDisk: "Scan Disk",
      rescan: "Scan Again",
      scanning: "Scanning…",
      startingScan: "Starting scan…",
      scanningPath: (path) => `Scanning ${path}…`,
      cleanSafeCount: (count) => `Clean Safe (${count})`,
      safeLabel: (size) => `${size} safe`,
      largestCulprits: "Largest Culprits",
      viewAll: "View all",
    },
    items: {
      emptyTitle: "No scan yet",
      emptyDesc: "Run a scan to inspect heavy folders and their subfolders.",
      filterAll: "All",
      cleanSafe: "Clean Safe",
      filterEmptyTitle: "No items in this filter",
      filterEmptyDesc: "Choose another risk level or scan again.",
      filesCount: (count) => `${count} files`,
      cleanableNote: (size) => ` · ${size} cleanable`,
      subfoldersCount: (count) => `Subfolders (${count})`,
      hide: "Hide",
      revealInFinder: "Reveal in Finder",
      ignoreAlways: "Ignore always",
      trash: "Trash",
      confirmTrashItem: (name, size) => `Move "${name}" (${size}) to Trash?`,
      confirmTrashSafe: (count, size) => `Move ${count} safe items (${size}) to Trash?`,
      cancel: "Cancel",
      moveToTrash: "Move to Trash",
      moving: "Moving…",
    },
    snapshots: {
      title: "APFS Snapshots",
      refreshAria: "Refresh APFS snapshots",
      loading: "Checking local Time Machine snapshots…",
      unsupported: "Local APFS snapshots are only available on macOS.",
      empty: "No local Time Machine snapshots found.",
      summaryLine: (count) =>
        count === 1
          ? "1 local Time Machine snapshot contributing to System Data."
          : `${count} local Time Machine snapshots contributing to System Data.`,
      hint: "Local Time Machine snapshots inflate System Data. Removal is permanent.",
      show: "View",
      hide: "Hide",
      clearAll: "Purge all",
      remove: "Delete",
      removing: "Deleting…",
      confirmSingle: (date) => `Permanently delete APFS snapshot from ${date}?`,
      confirmAll: (count) => `Permanently delete ${count} local APFS snapshots?`,
      confirmAction: "Delete Snapshot",
    },
    risk: {
      baixo: "Low",
      medio: "Medium",
      alto: "High",
    },
    settings: {
      loading: "Loading settings…",
      languageLabel: "Language",
      languageHint: "Sets the interface language and AI analysis summaries.",
      systemSectionLabel: "System & Menu Bar",
      autostartLabel: "Launch at login",
      autostartHint: "Opens Macleaner in the menu bar when macOS starts.",
      lowDiskAlertLabel: "Low disk alert (< 15% free)",
      lowDiskAlertHint: "Sends a native notification when free volume space drops below 15%.",
      allowlistLabel: "Ignored Folders (Allowlist)",
      allowlistEmpty: "No ignored folders yet. Use \"Ignore always\" in Items to skip folders during scans.",
      allowlistHint: "These folders are skipped during scans and AI analysis. Remove one to scan it again.",
      allowlistRestore: "Remove",
      minFolderSize: "Minimum folder size",
      minSizeAria: "Minimum size in megabytes",
      aiEngineLabel: "AI Engine (runs only during scan)",
      providerOllama: "Local AI (Ollama)",
      providerExternal: "External API (HTTPS)",
      providerDisabled: "Disabled (Rules only)",
      ollamaEndpoint: "Ollama endpoint",
      localModel: "Local model",
      autoModel: "Auto (detect installed model)",
      ollamaHint: "Analyzes real subfolders during scan and frees RAM right after.",
      externalEndpoint: "OpenAI-compatible endpoint (HTTPS)",
      model: "Model",
      apiKey: "API key",
      disabledHint: "Unmapped folders remain marked as High risk for safety.",
      detectModels: "Detect Models",
      detecting: "Detecting…",
      saveSettings: "Save Settings",
      saving: "Saving…",
    },
    toasts: {
      diskError: (err) => `Disk read error: ${err}`,
      scanComplete: (size) => `Scan finished: ${size} found`,
      scanFailed: (err) => `Scan failed: ${err}`,
      trashNoneMoved: (reason) => `Could not move: ${reason}`,
      trashPartial: (moved, failed) => `${moved} moved to Trash (${failed} failed)`,
      trashMoved: (size) => `${size} moved to Trash`,
      trashError: (err) => `Trash error: ${err}`,
      revealError: (err) => `Could not reveal in Finder: ${err}`,
      pathIgnored: (name) => `"${name}" added to allowlist`,
      pathUnignored: (name) => `"${name}" removed from allowlist — scan again to inspect`,
      ignoreError: (err) => `Failed to update allowlist: ${err}`,
      autostartError: (err) => `Failed to update launch at login: ${err}`,
      snapshotsLoadError: (err) => `Failed to list APFS snapshots: ${err}`,
      snapshotsDeleted: (count) =>
        count === 1 ? "1 APFS snapshot deleted" : `${count} APFS snapshots deleted`,
      snapshotsPartial: (deleted, failed) =>
        `${deleted} snapshots deleted (${failed} failed)`,
      snapshotsCancelled: "Snapshot deletion canceled",
      snapshotsError: (err) => `Snapshot deletion error: ${err}`,
      updateError: (err) => `Update failed: ${err}`,
      updateInstalling: "Downloading and installing update…",
      updateRestarting: "Update ready. Restarting Macleaner…",
      updateAvailable: (latest, current) => `New version v${latest} available (current: v${current})`,
      updateLatest: (version) => `You're on the latest version (v${version})`,
      settingsSaved: "Settings saved",
      modelsDetected: (list) => `Detected models: ${list}`,
      noModelsFound: "No models installed in Ollama",
    },
    days: {
      today: "today",
      oneDay: "1 day ago",
      nDays: (n) => `${n} days ago`,
    },
    dev: {
      subtabs: {
        projects: "Projects",
        xcode: "Xcode",
        docker: "Docker",
        tools: "Tools",
      },
      refreshAria: "Refresh Dev section",
      loading: "Inspecting environment…",
      loadError: (err) => `Inspection error: ${err}`,
      cleanAction: "Clean",
      cleaningAction: "Cleaning…",
      deleteAction: "Delete",
      deletingAction: "Deleting…",
      runAction: "Run",
      runningAction: "Running…",
      projects: {
        rootsTitle: "Dev folders",
        addFolder: "Folder",
        scan: "Scan",
        scanning: "Scanning…",
        removeRootAria: (name) => `Remove folder ${name}`,
        noRootsHint: "Add a code folder (e.g. ~/Projects) to locate node_modules, target, and .venv.",
        noRootsTitle: "No folders configured",
        noRootsDesc: "Add your project folders to discover reclaimable build artifacts.",
        filterAll: "All",
        filter30: ">30d",
        filter90: ">90d",
        cleanOlder: (days) => `Clean >${days}d`,
        emptyTitle: "No artifacts found",
        emptyDesc: "No heavy build artifacts (node_modules, target, .venv, build) found in watched folders.",
        filterEmptyTitle: "No projects in this filter",
        filterEmptyDesc: "Pick another inactivity window or rescan your folders.",
        summaryLine: (count, size) => `${count} projects · ${size}`,
        inactiveShort: (days) => (days === 0 ? "today" : `${days}d`),
        confirmTrashArtifact: (artifact, project, size) =>
          `Move "${artifact}" from ${project} (${size}) to Trash?`,
        confirmTrashBulk: (count, size, days) =>
          `Move ${count} artifacts from projects inactive >${days}d (${size}) to Trash?`,
        rootAdded: (name) => `Folder "${name}" added`,
        rootRemoved: (name) => `Folder "${name}" removed`,
        rootError: (err) => `Folder update failed: ${err}`,
        scanError: (err) => `Project scan failed: ${err}`,
      },
      xcode: {
        notInstalledTitle: "Xcode not detected",
        notInstalledDesc: "No Xcode data or simulators were found on this Mac.",
        summaryTitle: "Xcode & Simulators",
        allCleanTitle: "Xcode is clean",
        allCleanDesc: "No heavy caches, unavailable simulators, or runtimes found.",
        unavailableTitle: "Unavailable simulators",
        unavailableDesc: "Orphaned simulator devices from older Xcode releases.",
        cleanUnavailable: "Clean unavailable",
        runtimesTitle: "Simulator Runtimes",
        areasTitle: "Xcode Areas",
        categoryLabels: {
          derived_data: "DerivedData",
          archives: "Archives",
          device_support: "DeviceSupport",
          simulator_caches: "Caches",
          previews: "Previews",
        },
        moreCount: (n) => `+${n} more`,
        confirmUnavailable: (count, size) =>
          `Delete ${count} unavailable simulators (${size}) via simctl?`,
        confirmRuntime: (name, size) =>
          `Delete runtime "${name}" (${size}) via simctl?`,
        confirmTrashArea: (name, size) =>
          `Move "${name}" (${size}) to Trash?`,
        unavailableCleaned: (count, size) =>
          `${count} simulators deleted (${size})`,
        runtimeDeleted: (name, size) =>
          `Runtime "${name}" deleted (${size})`,
        actionError: (err) => `Xcode error: ${err}`,
      },
      docker: {
        notInstalledTitle: "Docker / OrbStack not detected",
        notInstalledDesc: "Install Docker Desktop or OrbStack to inspect images, containers, and caches.",
        stoppedTitle: "Daemon stopped",
        stoppedDesc: (engine) => `Start ${engine} to inspect and prune container resources.`,
        retry: "Retry",
        footprintLabel: "Disk footprint",
        reclaimableLabel: "Reclaimable",
        resourcesTitle: "Resources",
        activeOfTotal: (active, total) => `${active} active of ${total}`,
        reclaimablePct: (size, pct) => `${size} (${pct}%)`,
        clean: "Prune",
        cleanStatus: "Clean",
        readOnlyBadge: "Read-only",
        volumesSafetyNote: "Volumes may hold databases and are never pruned automatically.",
        kindLabels: {
          "build-cache": "Build Cache",
          images: "Images",
          containers: "Containers",
          volumes: "Local Volumes",
        },
        confirmPrune: (kindLabel, size) =>
          `Permanently prune ${kindLabel} (~${size} reclaimable)?`,
        pruneSuccess: (size) => `Docker pruned: ${size} freed`,
        pruneError: (err) => `Docker prune error: ${err}`,
      },
      tools: {
        brewTitle: "Homebrew",
        dryRunBadge: "Dry-run",
        brewNotInstalled: "Homebrew was not detected on this Mac.",
        brewClean: "Homebrew is already clean — no outdated items found.",
        brewOutdatedSummary: (count, size) => `${count} outdated items · ~${size} reclaimable`,
        showPreview: (count) => `Preview (${count})`,
        hidePreview: "Hide preview",
        runCleanup: "Run brew cleanup",
        autoremoveTitle: (count) => `${count} unused dependencies (autoremove)`,
        runAutoremove: "Run brew autoremove",
        cachesTitle: "Toolchain Caches",
        cachesEmpty: "No heavy toolchain caches detected.",
        confirmBrewCleanup: (size) =>
          `Run "brew cleanup" permanently (~${size} reclaimable)?`,
        confirmBrewAutoremove: (count) =>
          `Run "brew autoremove" to uninstall ${count} unused dependencies?`,
        confirmTrashCache: (name, size) =>
          `Move "${name}" cache (${size}) to Trash?`,
        brewSuccess: (action, size) =>
          `${action} completed (${size})`,
        brewError: (err) => `Homebrew error: ${err}`,
        cacheTrashed: (name, size) =>
          `"${name}" (${size}) moved to Trash`,
      },
    },
  },
  es: {
    htmlLang: "es",
    locale: "es-ES",
    tabs: {
      resumo: "Resumen",
      itens: "Ítems",
      dev: "Dev",
      ajustes: "Ajustes",
      navLabel: "Navegación principal",
    },
    confirm: {
      permanentWarning: "Acción permanente — no va a la Papelera",
    },
    header: {
      updateIdle: "Update",
      checkingUpdate: "Buscando…",
      installingUpdate: "Actualizando…",
      downloadingPct: (pct) => `Bajando ${pct}%…`,
      installVersion: (ver) => `Instalar v${ver}`,
      checkUpdateTitle: "Buscar actualizaciones",
      rescanAria: "Escanear disco de nuevo",
    },
    disk: {
      localFallback: "Disco Local",
      readingVolume: "Leyendo volumen…",
      healthy: "Saludable",
      warn: "Atención",
      critical: "Crítico",
      freeOf: (free, total) => `${free} libres de ${total}`,
      inUse: "En uso",
      mapped: "Mapeado",
    },
    summary: {
      smartCleanup: "Limpieza Inteligente",
      readyToScan: "Listo para escanear",
      foldersAnalyzed: (count) => `${count} carpetas pesadas analizadas`,
      idleHint: "Analiza cachés, Xcode, navegadores y subcarpetas con envío seguro a la Papelera.",
      scanDisk: "Escanear Disco",
      rescan: "Escanear de Nuevo",
      scanning: "Escaneando…",
      startingScan: "Iniciando escaneo…",
      scanningPath: (path) => `Escaneando ${path}…`,
      cleanSafeCount: (count) => `Limpiar Seguros (${count})`,
      safeLabel: (size) => `${size} seguros`,
      largestCulprits: "Mayores Ocupantes",
      viewAll: "Ver todos",
    },
    items: {
      emptyTitle: "Sin escaneo aún",
      emptyDesc: "Ejecuta un escaneo para listar las carpetas pesadas y subcarpetas.",
      filterAll: "Todos",
      cleanSafe: "Limpiar Seguros",
      filterEmptyTitle: "Sin elementos en este filtro",
      filterEmptyDesc: "Elige otro nivel de riesgo o escanea de nuevo.",
      filesCount: (count) => `${count} arch`,
      cleanableNote: (size) => ` · ${size} limpiables`,
      subfoldersCount: (count) => `Subcarpetas (${count})`,
      hide: "Ocultar",
      revealInFinder: "Mostrar en Finder",
      ignoreAlways: "Ignorar siempre",
      trash: "Papelera",
      confirmTrashItem: (name, size) => `¿Mover "${name}" (${size}) a la Papelera?`,
      confirmTrashSafe: (count, size) => `¿Mover ${count} elementos seguros (${size}) a la Papelera?`,
      cancel: "Cancelar",
      moveToTrash: "Mover a la Papelera",
      moving: "Moviendo…",
    },
    snapshots: {
      title: "Snapshots APFS",
      refreshAria: "Actualizar snapshots APFS",
      loading: "Comprobando snapshots locales de Time Machine…",
      unsupported: "Los snapshots APFS locales solo están disponibles en macOS.",
      empty: "No se encontraron snapshots locales de Time Machine.",
      summaryLine: (count) =>
        count === 1
          ? "1 snapshot local de Time Machine ocupando Datos del Sistema."
          : `${count} snapshots locales de Time Machine ocupando Datos del Sistema.`,
      hint: "Los snapshots locales inflan Datos del Sistema. Su eliminación es permanente.",
      show: "Ver",
      hide: "Ocultar",
      clearAll: "Limpiar todos",
      remove: "Eliminar",
      removing: "Eliminando…",
      confirmSingle: (date) => `¿Eliminar permanentemente el snapshot APFS de ${date}?`,
      confirmAll: (count) => `¿Eliminar permanentemente ${count} snapshots APFS locales?`,
      confirmAction: "Eliminar Snapshot",
    },
    risk: {
      baixo: "Bajo",
      medio: "Medio",
      alto: "Alto",
    },
    settings: {
      loading: "Cargando ajustes…",
      languageLabel: "Idioma",
      languageHint: "Define el idioma de la interfaz y del análisis de IA.",
      systemSectionLabel: "Sistema y Barra de Menús",
      autostartLabel: "Iniciar con macOS",
      autostartHint: "Abre Macleaner en la barra de menú al iniciar sesión.",
      lowDiskAlertLabel: "Alerta de poco espacio (< 15% libre)",
      lowDiskAlertHint: "Envía una notificación nativa cuando el volumen cae del 15% libre.",
      allowlistLabel: "Carpetas Ignoradas (Allowlist)",
      allowlistEmpty: "Ninguna carpeta ignorada. Usa \"Ignorar siempre\" en Ítems para omitir carpetas en el escaneo.",
      allowlistHint: "Estas carpetas se omiten en el escaneo y en la IA. Quita una para volver a analizarla.",
      allowlistRestore: "Quitar",
      minFolderSize: "Tamaño mínimo de carpeta",
      minSizeAria: "Tamaño mínimo en megabytes",
      aiEngineLabel: "Motor de IA (solo durante el escaneo)",
      providerOllama: "IA Local (Ollama)",
      providerExternal: "API Externa (HTTPS)",
      providerDisabled: "Desactivada (Solo reglas)",
      ollamaEndpoint: "Endpoint de Ollama",
      localModel: "Modelo local",
      autoModel: "Auto (detectar modelo instalado)",
      ollamaHint: "Analiza subcarpetas reales durante el escaneo y libera la RAM al finalizar.",
      externalEndpoint: "Endpoint OpenAI-compatible (HTTPS)",
      model: "Modelo",
      apiKey: "Clave de API",
      disabledHint: "Las carpetas sin regla conocida permanecen marcadas como riesgo Alto.",
      detectModels: "Detectar Modelos",
      detecting: "Detectando…",
      saveSettings: "Guardar Ajustes",
      saving: "Guardando…",
    },
    toasts: {
      diskError: (err) => `Error al leer disco: ${err}`,
      scanComplete: (size) => `Escaneo completado: ${size} encontrados`,
      scanFailed: (err) => `Fallo en el escaneo: ${err}`,
      trashNoneMoved: (reason) => `No se pudo mover: ${reason}`,
      trashPartial: (moved, failed) => `${moved} movidos a la Papelera (${failed} fallaron)`,
      trashMoved: (size) => `${size} movidos a la Papelera`,
      trashError: (err) => `Error al mover a la Papelera: ${err}`,
      revealError: (err) => `No se pudo abrir en Finder: ${err}`,
      pathIgnored: (name) => `"${name}" añadida a la lista de ignorados`,
      pathUnignored: (name) => `"${name}" quitada de ignorados — escanea de nuevo para verla`,
      ignoreError: (err) => `Error al actualizar ignorados: ${err}`,
      autostartError: (err) => `Error al configurar inicio automático: ${err}`,
      snapshotsLoadError: (err) => `Error al listar snapshots APFS: ${err}`,
      snapshotsDeleted: (count) =>
        count === 1 ? "1 snapshot APFS eliminado" : `${count} snapshots APFS eliminados`,
      snapshotsPartial: (deleted, failed) =>
        `${deleted} snapshots eliminados (${failed} fallaron)`,
      snapshotsCancelled: "Eliminación de snapshot cancelada",
      snapshotsError: (err) => `Error al eliminar snapshot APFS: ${err}`,
      updateError: (err) => `Error al actualizar: ${err}`,
      updateInstalling: "Descargando e instalando actualización…",
      updateRestarting: "Actualización lista. Reiniciando Macleaner…",
      updateAvailable: (latest, current) => `Nueva versión v${latest} disponible (actual: v${current})`,
      updateLatest: (version) => `Ya tienes la versión más reciente (v${version})`,
      settingsSaved: "Ajustes guardados",
      modelsDetected: (list) => `Modelos detectados: ${list}`,
      noModelsFound: "No se encontraron modelos en Ollama",
    },
    days: {
      today: "hoy",
      oneDay: "hace 1 día",
      nDays: (n) => `hace ${n} días`,
    },
    dev: {
      subtabs: {
        projects: "Proyectos",
        xcode: "Xcode",
        docker: "Docker",
        tools: "Tools",
      },
      refreshAria: "Actualizar sección Dev",
      loading: "Inspeccionando entorno…",
      loadError: (err) => `Error al inspeccionar: ${err}`,
      cleanAction: "Limpiar",
      cleaningAction: "Limpiando…",
      deleteAction: "Eliminar",
      deletingAction: "Eliminando…",
      runAction: "Ejecutar",
      runningAction: "Ejecutando…",
      projects: {
        rootsTitle: "Carpetas Dev",
        addFolder: "Carpeta",
        scan: "Escanear",
        scanning: "Escaneando…",
        removeRootAria: (name) => `Quitar carpeta ${name}`,
        noRootsHint: "Añade una carpeta de código (ej.: ~/Projects) para hallar node_modules, target y .venv.",
        noRootsTitle: "Sin carpetas configuradas",
        noRootsDesc: "Añade tus carpetas de proyectos para detectar artefactos de compilación recuperables.",
        filterAll: "Todos",
        filter30: ">30d",
        filter90: ">90d",
        cleanOlder: (days) => `Limpiar >${days}d`,
        emptyTitle: "Sin artefactos encontrados",
        emptyDesc: "No se hallaron artefactos pesados (node_modules, target, .venv, build) en las carpetas monitoreadas.",
        filterEmptyTitle: "Sin proyectos en este filtro",
        filterEmptyDesc: "Elige otro rango de inactividad o escanea de nuevo.",
        summaryLine: (count, size) => `${count} proyectos · ${size}`,
        inactiveShort: (days) => (days === 0 ? "hoy" : `${days}d`),
        confirmTrashArtifact: (artifact, project, size) =>
          `¿Mover "${artifact}" de ${project} (${size}) a la Papelera?`,
        confirmTrashBulk: (count, size, days) =>
          `¿Mover ${count} artefactos de proyectos inactivos >${days}d (${size}) a la Papelera?`,
        rootAdded: (name) => `Carpeta "${name}" añadida`,
        rootRemoved: (name) => `Carpeta "${name}" quitada`,
        rootError: (err) => `Error al gestionar carpetas: ${err}`,
        scanError: (err) => `Fallo al escanear proyectos: ${err}`,
      },
      xcode: {
        notInstalledTitle: "Xcode no detectado",
        notInstalledDesc: "No se encontraron datos de Xcode ni simuladores en este Mac.",
        summaryTitle: "Xcode y Simuladores",
        allCleanTitle: "Xcode limpio",
        allCleanDesc: "No se encontraron cachés pesadas, simuladores no disponibles ni runtimes.",
        unavailableTitle: "Simuladores no disponibles",
        unavailableDesc: "Dispositivos huérfanos de versiones anteriores de Xcode.",
        cleanUnavailable: "Limpiar no disponibles",
        runtimesTitle: "Runtimes de Simulador",
        areasTitle: "Áreas de Xcode",
        categoryLabels: {
          derived_data: "DerivedData",
          archives: "Archivos",
          device_support: "DeviceSupport",
          simulator_caches: "Cachés",
          previews: "Previews",
        },
        moreCount: (n) => `+${n} más`,
        confirmUnavailable: (count, size) =>
          `¿Eliminar ${count} simuladores no disponibles (${size}) vía simctl?`,
        confirmRuntime: (name, size) =>
          `¿Eliminar runtime "${name}" (${size}) vía simctl?`,
        confirmTrashArea: (name, size) =>
          `¿Mover "${name}" (${size}) a la Papelera?`,
        unavailableCleaned: (count, size) =>
          `${count} simuladores eliminados (${size})`,
        runtimeDeleted: (name, size) =>
          `Runtime "${name}" eliminado (${size})`,
        actionError: (err) => `Error en Xcode: ${err}`,
      },
      docker: {
        notInstalledTitle: "Docker / OrbStack no detectado",
        notInstalledDesc: "Instala Docker Desktop u OrbStack para inspeccionar imágenes, contenedores y cachés.",
        stoppedTitle: "Daemon detenido",
        stoppedDesc: (engine) => `Inicia ${engine} para inspeccionar y limpiar recursos de contenedores.`,
        retry: "Reintentar",
        footprintLabel: "Uso en disco",
        reclaimableLabel: "Recuperable",
        resourcesTitle: "Recursos",
        activeOfTotal: (active, total) => `${active} activos de ${total}`,
        reclaimablePct: (size, pct) => `${size} (${pct}%)`,
        clean: "Limpiar",
        cleanStatus: "Limpio",
        readOnlyBadge: "Solo lectura",
        volumesSafetyNote: "Los volúmenes pueden contener bases de datos y nunca se limpian automáticamente.",
        kindLabels: {
          "build-cache": "Build Cache",
          images: "Imágenes",
          containers: "Contenedores",
          volumes: "Volúmenes Locales",
        },
        confirmPrune: (kindLabel, size) =>
          `¿Ejecutar prune permanente en ${kindLabel} (~${size} recuperables)?`,
        pruneSuccess: (size) => `Docker limpio: ${size} liberados`,
        pruneError: (err) => `Error al limpiar Docker: ${err}`,
      },
      tools: {
        brewTitle: "Homebrew",
        dryRunBadge: "Dry-run",
        brewNotInstalled: "Homebrew no fue detectado en este Mac.",
        brewClean: "Homebrew ya está limpio — sin elementos obsoletos.",
        brewOutdatedSummary: (count, size) => `${count} elementos antiguos · ~${size} recuperables`,
        showPreview: (count) => `Vista previa (${count})`,
        hidePreview: "Ocultar vista previa",
        runCleanup: "Ejecutar brew cleanup",
        autoremoveTitle: (count) => `${count} dependencias huérfanas (autoremove)`,
        runAutoremove: "Ejecutar brew autoremove",
        cachesTitle: "Cachés de Toolchains",
        cachesEmpty: "No se detectaron cachés pesadas de toolchains.",
        confirmBrewCleanup: (size) =>
          `¿Ejecutar "brew cleanup" permanentemente (~${size} recuperables)?`,
        confirmBrewAutoremove: (count) =>
          `¿Ejecutar "brew autoremove" para quitar ${count} dependencias huérfanas?`,
        confirmTrashCache: (name, size) =>
          `¿Mover caché "${name}" (${size}) a la Papelera?`,
        brewSuccess: (action, size) =>
          `${action} completado (${size})`,
        brewError: (err) => `Error en Homebrew: ${err}`,
        cacheTrashed: (name, size) =>
          `"${name}" (${size}) movida a la Papelera`,
      },
    },
  },
};

export function getCopy(lang: AppLanguage): CopyBundle {
  return BUNDLES[lang] ?? BUNDLES["pt-br"];
}

export function formatInt(value: number, lang: AppLanguage): string {
  return new Intl.NumberFormat(getCopy(lang).locale).format(value);
}

export function formatBytes(bytes: number, lang: AppLanguage): string {
  if (bytes <= 0) {
    return "0\u00A0MB";
  }
  const locale = getCopy(lang).locale;
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let idx = 0;
  while (value >= 1024 && idx < units.length - 1) {
    value /= 1024;
    idx += 1;
  }
  const formatted =
    idx <= 1
      ? new Intl.NumberFormat(locale).format(Math.round(value))
      : new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(value);
  return `${formatted}\u00A0${units[idx]}`;
}

export function formatDays(days: number, lang: AppLanguage): string {
  const copy = getCopy(lang);
  if (days === 0) {
    return copy.days.today;
  }
  if (days === 1) {
    return copy.days.oneDay;
  }
  return copy.days.nDays(formatInt(days, lang));
}

export function formatSnapshotTimestamp(iso: string, lang: AppLanguage): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    return iso;
  }
  return new Intl.DateTimeFormat(getCopy(lang).locale, {
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

export function formatSnapshotAge(iso: string, lang: AppLanguage): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    return "";
  }
  const diffDays = Math.max(0, Math.floor((Date.now() - date.getTime()) / 86_400_000));
  return formatDays(diffDays, lang);
}
