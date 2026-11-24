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
    trash: string;
    confirmTrashItem: (name: string, size: string) => string;
    confirmTrashSafe: (count: number, size: string) => string;
    cancel: string;
    moveToTrash: string;
    moving: string;
  };
  risk: Record<RiskLevel, string>;
  settings: {
    loading: string;
    languageLabel: string;
    languageHint: string;
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
      trash: "Lixeira",
      confirmTrashItem: (name, size) => `Mover "${name}" (${size}) para a Lixeira?`,
      confirmTrashSafe: (count, size) => `Mover ${count} itens seguros (${size}) para a Lixeira?`,
      cancel: "Cancelar",
      moveToTrash: "Mover para Lixeira",
      moving: "Movendo…",
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
      trash: "Trash",
      confirmTrashItem: (name, size) => `Move "${name}" (${size}) to Trash?`,
      confirmTrashSafe: (count, size) => `Move ${count} safe items (${size}) to Trash?`,
      cancel: "Cancel",
      moveToTrash: "Move to Trash",
      moving: "Moving…",
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
      trash: "Papelera",
      confirmTrashItem: (name, size) => `¿Mover "${name}" (${size}) a la Papelera?`,
      confirmTrashSafe: (count, size) => `¿Mover ${count} elementos seguros (${size}) a la Papelera?`,
      cancel: "Cancelar",
      moveToTrash: "Mover a la Papelera",
      moving: "Moviendo…",
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
