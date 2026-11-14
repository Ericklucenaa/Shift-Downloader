import { ptBR, type MessageKey, type Messages } from './locales/pt-BR';
import { en } from './locales/en';
import { es } from './locales/es';
import type { Diagnostic, Language } from './types';
import { useApp } from './store';

export const LOCALES: Record<Language, Messages> = { 'pt-BR': ptBR, en, es };

export function translate(lang: Language, key: MessageKey, vars?: Record<string, string | number>): string {
  const msg = LOCALES[lang]?.[key] ?? ptBR[key] ?? key;
  if (!vars) return msg;
  return msg.replace(/\{(\w+)\}/g, (m, k: string) => (k in vars ? String(vars[k]) : m));
}

export type T = (key: MessageKey, vars?: Record<string, string | number>) => string;

export function useT(): { t: T; lang: Language } {
  const lang = useApp((s) => s.settings?.language ?? 'pt-BR');
  return { t: (key, vars) => translate(lang, key, vars), lang };
}

// Diagnósticos vêm do núcleo em português. Para inglês e espanhol, textos por tipo.
const DIAG: Partial<Record<Language, Partial<Record<Diagnostic['kind'], [string, string]>>>> = {
  en: {
    dns: ['Could not find the server', 'Check the address and your internet connection.'],
    connection: ['The connection to the server dropped', 'Check your internet. SHIFT retries on its own and continues where it stopped.'],
    timeout: ['The server took too long to respond', 'SHIFT retries on its own. If it persists, the server may be overloaded.'],
    tls: ['The secure connection could not be verified', 'The site certificate is invalid or the computer clock is wrong. Do not continue unless you trust the site.'],
    http_auth: ['The server asked for a login', 'This file requires authentication. Download it in the browser with your account or use a public link.'],
    forbidden: ['The server denied access (403)', 'Download links often expire. Get a new link from the source page and try again.'],
    not_found: ['The file is no longer at this address (404)', 'The file moved or the link expired. Copy the link again from the source page.'],
    expired: ['The link expired (410)', 'Get a new link from the source page and add the download again.'],
    server_error: ['Server error', 'A problem on the site’s side. SHIFT retries on its own shortly.'],
    server_busy: ['Server busy', 'The site is limiting requests for now. SHIFT waits and retries.'],
    too_slow: ['Speed dropped below the minimum', 'SHIFT retries. If it persists, lower the connections in settings.'],
    disk_full: ['Not enough disk space', 'Free up space or choose another folder. Existing files were not touched.'],
    permission: ['No permission to write to the folder', 'Choose another destination folder or check its permissions.'],
    file_exists: ['A file with this name already exists', 'Choose another name. SHIFT never overwrites existing files.'],
    io: ['Error writing the file', 'Check the destination disk (disconnected, full or failing) and try again.'],
    resume_unsupported: ['The server does not allow resuming', 'The download has to start over.'],
    remote_changed: ['The file changed on the server', 'The part already downloaded belonged to another version and was discarded. The download starts over.'],
    redirect_blocked: ['Redirect blocked', 'The site tried to send the download to an internal address on your network. This was blocked for safety.'],
    too_many_redirects: ['Too many redirects', 'The link loops between redirects. Copy the final file link from the browser.'],
    checksum: ['The downloaded file does not match', 'The integrity check failed. Download it again.'],
    invalid_url: ['Invalid address', 'Paste the full file address, starting with http:// or https://.'],
    engine: ['The download engine stopped', 'SHIFT is restarting the engine. The download continues when it is back.'],
    unsupported_url: ['This site is not supported for video/audio', 'If it is a direct file link, paste it in the downloads bar instead of the media area.'],
    drm: ['DRM-protected content', 'SHIFT does not bypass content protection. Use the official app or website.'],
    format_unavailable: ['The chosen format is not available', 'Analyze the link again and choose another quality.'],
    media_failed: ['Media processing failed', 'The original file was not changed. See the technical detail and try another format.'],
    tool_missing: ['Media tool not found', 'Reinstall SHIFT: yt-dlp or FFmpeg is missing from the program folder.'],
    unknown: ['The download failed', 'Try again. The technical details below help explain the problem.']
  },
  es: {
    dns: ['No se encontró el servidor', 'Revisa la dirección y tu conexión a internet.'],
    connection: ['Se cayó la conexión con el servidor', 'Revisa tu internet. SHIFT reintenta solo y continúa donde se detuvo.'],
    timeout: ['El servidor tardó demasiado en responder', 'SHIFT reintenta solo. Si continúa, el servidor puede estar saturado.'],
    tls: ['No se pudo verificar la conexión segura', 'El certificado del sitio no es válido o la hora del equipo es incorrecta. No continúes si no confías en el sitio.'],
    http_auth: ['El servidor pidió iniciar sesión', 'Este archivo requiere autenticación. Descárgalo en el navegador con tu cuenta o usa un enlace público.'],
    forbidden: ['El servidor negó el acceso (403)', 'Los enlaces de descarga suelen caducar. Obtén un enlace nuevo en la página de origen y reintenta.'],
    not_found: ['El archivo ya no está en esta dirección (404)', 'El archivo se movió o el enlace caducó. Copia el enlace de nuevo en la página de origen.'],
    expired: ['El enlace caducó (410)', 'Obtén un enlace nuevo en la página de origen y añade la descarga otra vez.'],
    server_error: ['Error del servidor', 'Problema del lado del sitio. SHIFT reintenta solo en unos instantes.'],
    server_busy: ['Servidor ocupado', 'El sitio está limitando las solicitudes. SHIFT espera y reintenta.'],
    too_slow: ['La velocidad bajó del mínimo', 'SHIFT reintenta. Si continúa, reduce las conexiones en Ajustes.'],
    disk_full: ['Espacio insuficiente en el disco', 'Libera espacio o elige otra carpeta. Los archivos existentes no se modificaron.'],
    permission: ['Sin permiso para escribir en la carpeta', 'Elige otra carpeta de destino o revisa sus permisos.'],
    file_exists: ['Ya existe un archivo con ese nombre', 'Elige otro nombre. SHIFT nunca sobrescribe archivos.'],
    io: ['Error al escribir el archivo', 'Revisa el disco de destino (desconectado, lleno o con fallos) y reintenta.'],
    resume_unsupported: ['El servidor no permite reanudar', 'La descarga tiene que empezar de nuevo.'],
    remote_changed: ['El archivo cambió en el servidor', 'La parte descargada era de otra versión y se descartó. La descarga empieza de nuevo.'],
    redirect_blocked: ['Redirección bloqueada', 'El sitio intentó llevar la descarga a una dirección interna de tu red. Se impidió por seguridad.'],
    too_many_redirects: ['Demasiadas redirecciones', 'El enlace entra en un ciclo de redirecciones. Copia el enlace final del archivo desde el navegador.'],
    checksum: ['El archivo descargado no coincide', 'La verificación de integridad falló. Descárgalo de nuevo.'],
    invalid_url: ['Dirección no válida', 'Pega la dirección completa del archivo, empezando con http:// o https://.'],
    engine: ['El motor de descargas se detuvo', 'SHIFT está reiniciando el motor. La descarga continúa cuando vuelva.'],
    unsupported_url: ['Este sitio no es compatible para video/audio', 'Si es un enlace directo a un archivo, pégalo en la barra de descargas.'],
    drm: ['Contenido protegido con DRM', 'SHIFT no evita la protección de contenido. Usa la aplicación o el sitio oficial.'],
    format_unavailable: ['El formato elegido no está disponible', 'Analiza el enlace de nuevo y elige otra calidad.'],
    media_failed: ['Falló el procesamiento del medio', 'El original no se modificó. Revisa el detalle técnico y prueba otro formato.'],
    tool_missing: ['Herramienta de medios no encontrada', 'Reinstala SHIFT: falta yt-dlp o FFmpeg en la carpeta del programa.'],
    unknown: ['La descarga falló', 'Reintenta. Los detalles técnicos de abajo ayudan a entender el problema.']
  }
};

export function diagnosticText(d: Diagnostic, lang: Language): { title: string; action: string } {
  // 'unknown' carrega mensagens específicas do núcleo (ex.: "Pause antes de remover"): mantém o texto.
  const loc = d.kind === 'unknown' ? undefined : DIAG[lang]?.[d.kind];
  return loc ? { title: loc[0], action: loc[1] } : { title: d.title, action: d.action };
}
