import type { AppLanguage, RiskLevel } from "./types";

export interface CopyBundle {
  htmlLang: string;
  locale: string;
  tabs: {
    resumo: string;
    itens: string;
    ajustes: string;
    navLabel: string;
  };
  header: {
    updateIdle: string;
    checkingUpdate: string;
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
}

const BUNDLES: Record<AppLanguage, CopyBundle> = {
  "pt-br": {
    htmlLang: "pt-BR",
    locale: "pt-BR",
    tabs: {
      resumo: "Resumo",
      itens: "Itens",
      ajustes: "Ajustes",
      navLabel: "Navegação principal",
    },
    header: {
      updateIdle: "Update",
      checkingUpdate: "Checando…",
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
      confirmSingle: (date) =>
        `Remover permanentemente o snapshot APFS de ${date}? (Não vai para a Lixeira)`,
      confirmAll: (count) =>
        `Remover permanentemente ${count} snapshots APFS locais? (Não vão para a Lixeira)`,
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
      updateError: (err) => `Erro ao verificar atualização: ${err}`,
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
  },
  en: {
    htmlLang: "en",
    locale: "en-US",
    tabs: {
      resumo: "Summary",
      itens: "Items",
      ajustes: "Settings",
      navLabel: "Main navigation",
    },
    header: {
      updateIdle: "Update",
      checkingUpdate: "Checking…",
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
      confirmSingle: (date) =>
        `Permanently delete APFS snapshot from ${date}? (Does not go to Trash)`,
      confirmAll: (count) =>
        `Permanently delete ${count} local APFS snapshots? (Cannot be undone via Trash)`,
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
      updateError: (err) => `Update check failed: ${err}`,
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
  },
  es: {
    htmlLang: "es",
    locale: "es-ES",
    tabs: {
      resumo: "Resumen",
      itens: "Elementos",
      ajustes: "Ajustes",
      navLabel: "Navegación principal",
    },
    header: {
      updateIdle: "Update",
      checkingUpdate: "Buscando…",
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
      confirmSingle: (date) =>
        `¿Eliminar permanentemente el snapshot APFS de ${date}? (No va a la Papelera)`,
      confirmAll: (count) =>
        `¿Eliminar permanentemente ${count} snapshots APFS locales? (No van a la Papelera)`,
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
      allowlistEmpty: "Ninguna carpeta ignorada. Usa \"Ignorar siempre\" en Elementos para omitir carpetas en el escaneo.",
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
      updateError: (err) => `Error al buscar actualizaciones: ${err}`,
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
