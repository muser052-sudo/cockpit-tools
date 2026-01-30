
const fs = require('fs');
const path = require('path');

const localesDir = path.join(__dirname, '..', 'src', 'locales');

// Full translations including settings.general keys
const translations = {
  ar: {
    codex_oauth_openBrowser: "فتح في المتصفح",
    codex_oauth_hint: "بمجرد التفويض ، سيتم تحديث هذه النافذة تلقائيًا",
    codex_token_import: "استيراد",
    codex_local_import: "الحصول على الحساب المحلي",
    codex_oauth_portInUseAction: "إغلاق المنفذ والمحاولة مرة أخرى",
    update_notification_whatsNew: "ما الجديد",
    accounts_confirmDeleteTag: "هل تريد حذف العلامة \"{{tag}}\"؟ ستتم إزالة هذه العلامة من {{count}} حسابات.",
    accounts_defaultGroup: "مجموعة افتراضية",
    settings_general_closeBehavior: "سلوك الإغلاق",
    settings_general_closeBehaviorDesc: "اختر الإجراء عند إغلاق النافذة",
    settings_general_closeBehaviorAsk: "اسأل في كل مرة",
    settings_general_closeBehaviorMinimize: "تصغير إلى الدرج",
    settings_general_closeBehaviorQuit: "إنهاء التطبيق"
  },
  cs: {
    codex_oauth_openBrowser: "Otevřít v prohlížeči",
    codex_oauth_hint: "Po autorizaci se toto okno automaticky aktualizuje",
    codex_token_import: "Importovat",
    codex_local_import: "Získat místní účet",
    codex_oauth_portInUseAction: "Zavřít port a zkusit to znovu",
    update_notification_whatsNew: "Co je nového",
    accounts_confirmDeleteTag: "Smazat štítek \"{{tag}}\"? Tento štítek bude odebrán z {{count}} účtů.",
    accounts_defaultGroup: "Výchozí skupina",
    settings_general_closeBehavior: "Chování při zavírání",
    settings_general_closeBehaviorDesc: "Vyberte akci při zavření okna",
    settings_general_closeBehaviorAsk: "Vždy se zeptat",
    settings_general_closeBehaviorMinimize: "Minimalizovat do lišty",
    settings_general_closeBehaviorQuit: "Ukončit aplikaci"
  },
  de: {
    codex_oauth_openBrowser: "Im Browser öffnen",
    codex_oauth_hint: "Nach der Autorisierung wird dieses Fenster automatisch aktualisiert",
    codex_token_import: "Importieren",
    codex_local_import: "Lokales Konto abrufen",
    codex_oauth_portInUseAction: "Port schließen und erneut versuchen",
    update_notification_whatsNew: "Was ist neu",
    accounts_confirmDeleteTag: "Tag \"{{tag}}\" löschen? Dieser Tag wird von {{count}} Konten entfernt.",
    accounts_defaultGroup: "Standardgruppe",
    settings_general_closeBehavior: "Verhalten beim Schließen",
    settings_general_closeBehaviorDesc: "Aktion beim Schließen des Fensters wählen",
    settings_general_closeBehaviorAsk: "Jedes Mal fragen",
    settings_general_closeBehaviorMinimize: "In den Tray minimieren",
    settings_general_closeBehaviorQuit: "Anwendung beenden"
  },
  "en-US": { // Same as en
    codex_oauth_openBrowser: "Open in Browser",
    codex_oauth_hint: "Once authorized, this window will update automatically",
    codex_token_import: "Import",
    codex_local_import: "Get Local Account",
    codex_oauth_portInUseAction: "Close port and retry",
    update_notification_whatsNew: "What's New",
    accounts_confirmDeleteTag: "Delete tag \"{{tag}}\"? This tag will be removed from {{count}} accounts.",
    accounts_defaultGroup: "Default Group",
    settings_general_closeBehavior: "Close Behavior",
    settings_general_closeBehaviorDesc: "Choose action when closing window",
    settings_general_closeBehaviorAsk: "Ask every time",
    settings_general_closeBehaviorMinimize: "Minimize to tray",
    settings_general_closeBehaviorQuit: "Quit application"
  },
  es: {
    codex_oauth_openBrowser: "Abrir en el navegador",
    codex_oauth_hint: "Una vez autorizado, esta ventana se actualizará automáticamente",
    codex_token_import: "Importar",
    codex_local_import: "Obtener cuenta local",
    codex_oauth_portInUseAction: "Cerrar puerto y reintentar",
    update_notification_whatsNew: "Novedades",
    accounts_confirmDeleteTag: "¿Eliminar etiqueta \"{{tag}}\"? Esta etiqueta se eliminará de {{count}} cuentas.",
    accounts_defaultGroup: "Grupo predeterminado",
    settings_general_closeBehavior: "Comportamiento al cerrar",
    settings_general_closeBehaviorDesc: "Elegir acción al cerrar la ventana",
    settings_general_closeBehaviorAsk: "Preguntar siempre",
    settings_general_closeBehaviorMinimize: "Minimizar a la bandeja",
    settings_general_closeBehaviorQuit: "Salir de la aplicación"
  },
  fr: {
    codex_oauth_openBrowser: "Ouvrir dans le navigateur",
    codex_oauth_hint: "Une fois autorisé, cette fenêtre se mettra à jour automatiquement",
    codex_token_import: "Importer",
    codex_local_import: "Obtener le compte local",
    codex_oauth_portInUseAction: "Fermer le port et réessayer",
    update_notification_whatsNew: "Nouveautés",
    accounts_confirmDeleteTag: "Supprimer l'étiquette \"{{tag}}\" ? Cette étiquette sera supprimée de {{count}} comptes.",
    accounts_defaultGroup: "Groupe par défaut",
    settings_general_closeBehavior: "Comportement à la fermeture",
    settings_general_closeBehaviorDesc: "Action à la fermeture de la fenêtre",
    settings_general_closeBehaviorAsk: "Demander à chaque fois",
    settings_general_closeBehaviorMinimize: "Minimiser dans la barre d'état",
    settings_general_closeBehaviorQuit: "Quitter l'application"
  },
  it: {
    codex_oauth_openBrowser: "Apri nel browser",
    codex_oauth_hint: "Una volta autorizzato, questa finestra si aggiornerà automaticamente",
    codex_token_import: "Importa",
    codex_local_import: "Ottieni account locale",
    codex_oauth_portInUseAction: "Chiudi porta e riprova",
    update_notification_whatsNew: "Novità",
    accounts_confirmDeleteTag: "Eliminare il tag \"{{tag}}\"? Questo tag verrà rimosso da {{count}} account.",
    accounts_defaultGroup: "Gruppo predefinito",
    settings_general_closeBehavior: "Comportamento alla chiusura",
    settings_general_closeBehaviorDesc: "Scegli azione alla chiusura della finestra",
    settings_general_closeBehaviorAsk: "Chiedi ogni volta",
    settings_general_closeBehaviorMinimize: "Riduci a icona nel vassoio",
    settings_general_closeBehaviorQuit: "Esci dall'applicazione"
  },
  ja: {
    codex_oauth_openBrowser: "ブラウザで開く",
    codex_oauth_hint: "認証が完了すると、このウィンドウは自動的に更新されます",
    codex_token_import: "インポート",
    codex_local_import: "ローカルアカウントを取得",
    codex_oauth_portInUseAction: "ポートを閉じて再試行",
    update_notification_whatsNew: "新着情報",
    accounts_confirmDeleteTag: "タグ「{{tag}}」を削除しますか？このタグは {{count}} 個のアカウントから削除されます。",
    accounts_defaultGroup: "デフォルトグループ",
    settings_general_closeBehavior: "閉じる時の動作",
    settings_general_closeBehaviorDesc: "ウィンドウを閉じる時の動作を選択",
    settings_general_closeBehaviorAsk: "毎回確認する",
    settings_general_closeBehaviorMinimize: "トレイに最小化",
    settings_general_closeBehaviorQuit: "アプリを終了"
  },
  ko: {
    codex_oauth_openBrowser: "브라우저에서 열기",
    codex_oauth_hint: "승인되면 이 창이 자동으로 업데이트됩니다",
    codex_token_import: "가져오기",
    codex_local_import: "로컬 계정 가져오기",
    codex_oauth_portInUseAction: "포트 닫기 및 재시도",
    update_notification_whatsNew: "새로운 기능",
    accounts_confirmDeleteTag: "\"{{tag}}\" 태그를 삭제하시겠습니까? 이 태그는 {{count}}개의 계정에서 제거됩니다.",
    accounts_defaultGroup: "기본 그룹",
    settings_general_closeBehavior: "닫기 동작",
    settings_general_closeBehaviorDesc: "창을 닫을 때의 동작 선택",
    settings_general_closeBehaviorAsk: "항상 묻기",
    settings_general_closeBehaviorMinimize: "트레이로 최소화",
    settings_general_closeBehaviorQuit: "애플리케이션 종료"
  },
  pl: {
    codex_oauth_openBrowser: "Otwórz w przeglądarce",
    codex_oauth_hint: "Po autoryzacji to okno zaktualizuje się automatycznie",
    codex_token_import: "Importuj",
    codex_local_import: "Pobierz konto lokalne",
    codex_oauth_portInUseAction: "Zamknij port i spróbuj ponownie",
    update_notification_whatsNew: "Co nowego",
    accounts_confirmDeleteTag: "Usunąć tag \"{{tag}}\"? Ten tag zostanie usunięty z {{count}} kont.",
    accounts_defaultGroup: "Grupa domyślna",
    settings_general_closeBehavior: "Zachowanie przy zamykaniu",
    settings_general_closeBehaviorDesc: "Wybierz akcję przy zamykaniu okna",
    settings_general_closeBehaviorAsk: "Zawsze pytaj",
    settings_general_closeBehaviorMinimize: "Minimalizuj do zasobnika",
    settings_general_closeBehaviorQuit: "Zamknij aplikację"
  },
  "pt-br": {
    codex_oauth_openBrowser: "Abrir no navegador",
    codex_oauth_hint: "Uma vez autorizado, esta janela será atualizada automaticamente",
    codex_token_import: "Importar",
    codex_local_import: "Obter conta local",
    codex_oauth_portInUseAction: "Fechar porta e tentar novamente",
    update_notification_whatsNew: "O que há de novo",
    accounts_confirmDeleteTag: "Excluir tag \"{{tag}}\"? Esta tag será removida de {{count}} contas.",
    accounts_defaultGroup: "Grupo padrão",
    settings_general_closeBehavior: "Comportamento ao fechar",
    settings_general_closeBehaviorDesc: "Escolha a ação ao fechar a janela",
    settings_general_closeBehaviorAsk: "Perguntar sempre",
    settings_general_closeBehaviorMinimize: "Minimizar para a bandeja",
    settings_general_closeBehaviorQuit: "Sair do aplicativo"
  },
  ru: {
    codex_oauth_openBrowser: "Открыть в браузере",
    codex_oauth_hint: "После авторизации это окно обновится автоматически",
    codex_token_import: "Импорт",
    codex_local_import: "Получить локальный аккаунт",
    codex_oauth_portInUseAction: "Закрыть порт и повторить",
    update_notification_whatsNew: "Что нового",
    accounts_confirmDeleteTag: "Удалить тег \"{{tag}}\"? Этот тег будет удален из {{count}} аккаунтов.",
    accounts_defaultGroup: "Группа по умолчанию",
    settings_general_closeBehavior: "Поведение при закрытии",
    settings_general_closeBehaviorDesc: "Действие при закрытии окна",
    settings_general_closeBehaviorAsk: "Спрашивать каждый раз",
    settings_general_closeBehaviorMinimize: "Свернуть в трей",
    settings_general_closeBehaviorQuit: "Закрыть приложение"
  },
  tr: {
    codex_oauth_openBrowser: "Tarayıcıda aç",
    codex_oauth_hint: "Yetkilendirildikten sonra bu pencere otomatik olarak güncellenecektir",
    codex_token_import: "İçe aktar",
    codex_local_import: "Yerel Hesabı Al",
    codex_oauth_portInUseAction: "Bağlantı noktasını kapat ve tekrar dene",
    update_notification_whatsNew: "Yenilikler",
    accounts_confirmDeleteTag: "\"{{tag}}\" etiketi silinsin mi? Bu etiket {{count}} hesaptan kaldırılacak.",
    accounts_defaultGroup: "Varsayılan Grup",
    settings_general_closeBehavior: "Kapanış Davranışı",
    settings_general_closeBehaviorDesc: "Pencere kapatıldığında yapılacak işlem",
    settings_general_closeBehaviorAsk: "Her seferinde sor",
    settings_general_closeBehaviorMinimize: "Tepsisine küçült",
    settings_general_closeBehaviorQuit: "Uygulamadan Çık"
  },
  vi: {
    codex_oauth_openBrowser: "Mở trong trình duyệt",
    codex_oauth_hint: "Sau khi được ủy quyền, cửa sổ này sẽ tự động cập nhật",
    codex_token_import: "Nhập",
    codex_local_import: "Lấy tài khoản cục bộ",
    codex_oauth_portInUseAction: "Đóng cổng và thử lại",
    update_notification_whatsNew: "Có gì mới",
    accounts_confirmDeleteTag: "Xóa thẻ \"{{tag}}\"? Thẻ này sẽ bị xóa khỏi {{count}} tài khoản.",
    accounts_defaultGroup: "Nhóm mặc định",
    settings_general_closeBehavior: "Hành động khi đóng",
    settings_general_closeBehaviorDesc: "Chọn hành động khi đóng cửa sổ",
    settings_general_closeBehaviorAsk: "Hỏi mỗi lần",
    settings_general_closeBehaviorMinimize: "Thu nhỏ xuống khay",
    settings_general_closeBehaviorQuit: "Thoát ứng dụng"
  },
  "zh-tw": {
    codex_oauth_openBrowser: "在瀏覽器中開啟",
    codex_oauth_hint: "完成授權後，此視窗將自動更新",
    codex_token_import: "匯入",
    codex_local_import: "獲取本機帳號",
    codex_oauth_portInUseAction: "關閉連接埠並重試",
    update_notification_whatsNew: "更新內容",
    accounts_confirmDeleteTag: "確認刪除標籤 \"{{tag}}\" 嗎？該標籤將從 {{count}} 個帳號中移除。",
    accounts_defaultGroup: "預設分組",
    settings_general_closeBehavior: "視窗關閉行為",
    settings_general_closeBehaviorDesc: "選擇關閉視窗時的預設行為",
    settings_general_closeBehaviorAsk: "每次詢問",
    settings_general_closeBehaviorMinimize: "最小化到系統列",
    settings_general_closeBehaviorQuit: "退出應用程式"
  }
};

const ignoredFiles = ['en.json', 'zh-CN.json'];

function updateFile(fileName) {
  if (ignoredFiles.includes(fileName)) return;

  const code = fileName.replace('.json', '');
  const trans = translations[code];
  
  // Use en-US fallback if language not found
  const actualTrans = trans || translations['en-US'];

  const filePath = path.join(localesDir, fileName);
  if (!fs.existsSync(filePath)) return;

  try {
    const content = JSON.parse(fs.readFileSync(filePath, 'utf8'));
    let modified = false;

    // Helper to safely set nested keys
    const setKey = (obj, path, value) => {
      const keys = path.split('.');
      let current = obj;
      for (let i = 0; i < keys.length - 1; i++) {
        if (!current[keys[i]]) current[keys[i]] = {};
        current = current[keys[i]];
      }
      if (!current[keys[keys.length - 1]]) {
        current[keys[keys.length - 1]] = value;
        return true;
      }
      return false;
    };

    if(setKey(content, 'codex.oauth.openBrowser', actualTrans.codex_oauth_openBrowser)) modified = true;
    if(setKey(content, 'codex.oauth.hint', actualTrans.codex_oauth_hint)) modified = true;
    if(setKey(content, 'codex.token.import', actualTrans.codex_token_import)) modified = true;
    if(setKey(content, 'codex.local.import', actualTrans.codex_local_import)) modified = true;
    if(setKey(content, 'codex.oauth.portInUseAction', actualTrans.codex_oauth_portInUseAction)) modified = true;
    if(setKey(content, 'update_notification.whatsNew', actualTrans.update_notification_whatsNew)) modified = true;
    if(setKey(content, 'accounts.confirmDeleteTag', actualTrans.accounts_confirmDeleteTag)) modified = true;
    if(setKey(content, 'accounts.defaultGroup', actualTrans.accounts_defaultGroup)) modified = true;
    
    // New Settings keys
    if(setKey(content, 'settings.general.closeBehavior', actualTrans.settings_general_closeBehavior)) modified = true;
    if(setKey(content, 'settings.general.closeBehaviorDesc', actualTrans.settings_general_closeBehaviorDesc)) modified = true;
    if(setKey(content, 'settings.general.closeBehaviorAsk', actualTrans.settings_general_closeBehaviorAsk)) modified = true;
    if(setKey(content, 'settings.general.closeBehaviorMinimize', actualTrans.settings_general_closeBehaviorMinimize)) modified = true;
    if(setKey(content, 'settings.general.closeBehaviorQuit', actualTrans.settings_general_closeBehaviorQuit)) modified = true;

    if (modified) {
      fs.writeFileSync(filePath, JSON.stringify(content, null, 2));
      console.log(`Updated ${fileName}`);
    } else {
      console.log(`No changes needed for ${fileName}`);
    }

  } catch (e) {
    console.error(`Error updating ${fileName}:`, e);
  }
}

const files = fs.readdirSync(localesDir);
files.forEach(file => {
  if (file.endsWith('.json')) {
    updateFile(file);
  }
});
