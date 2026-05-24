// Книга Inkhaven — главный файл (русское издание).
//
// Сборка:
//   typst compile Book-ru/BOOK_OF_INKHAVEN.typ
//
// Результат: Book-ru/BOOK_OF_INKHAVEN.pdf
//
// Каждая глава лежит в chapters/ отдельным .typ-файлом.
// Порядок глав отражает путь «от простого к сложному»:
// установка + первая книга → редактор → построение мира
// → ИИ → продвинутый Typst → скриптинг.

#import "design.typ": *

// ── Текущее состояние перевода ─────────────────────────
// Переведено: пролог + глава 1. Остальные главы переводятся
// поглавно; этот файл будет дополняться по мере появления
// готовых переводов. Закомментированные строки ниже
// показывают полный план структуры книги.

#book((
  include "chapters/00-prologue.typ",

  // Часть I — Основы
  include "chapters/01-what-inkhaven-is.typ",
  //   include "chapters/02-installation.typ",
  //   include "chapters/03-the-project-tree.typ",
  //   include "chapters/04-system-books.typ",
  //   include "chapters/05-configuration.typ",
  //
  // Часть II — Редактор
  //   include "chapters/06-writing-in-typst.typ",
  //   include "chapters/07-editor-workflow.typ",
  //   include "chapters/08-saving-snapshots.typ",
  //   include "chapters/09-status-and-goals.typ",
  //
  // Часть III — Поиск, резервные копии, экспорт
  //   include "chapters/10-search-and-discovery.typ",
  //   include "chapters/11-backups-and-recovery.typ",
  //   include "chapters/12-exporting.typ",
  //
  // Часть IV — Построение мира
  //   include "chapters/13-places-and-characters.typ",
  //   include "chapters/14-tags.typ",
  //   include "chapters/15-wiki-links.typ",
  //   include "chapters/16-story-view.typ",
  //
  // Часть V — Хронология
  //   include "chapters/17-story-timeline.typ",
  //
  // Часть VI — Работа с ИИ
  //   include "chapters/18-ai-providers.typ",
  //   include "chapters/19-the-ai-pane.typ",
  //   include "chapters/20-prompts-and-grammar.typ",
  //   include "chapters/21-critique-and-memory.typ",
  //   include "chapters/22-ai-for-diagnostics-and-timeline.typ",
  //
  // Часть VII — Освоение Typst
  //   include "chapters/23-typst-inside-inkhaven.typ",
  //   include "chapters/24-diagnostics-and-render.typ",
  //   include "chapters/25-multi-format-export.typ",
  //
  // Часть VIII — Импорт
  //   include "chapters/26-importing-existing-work.typ",
  //
  // Часть IX — Настройка и скриптинг
  //   include "chapters/27-theming.typ",
  //   include "chapters/28-reassigning-keys.typ",
  //   include "chapters/29-bund-scripting.typ",
  //
  // Приложения
  //   include "chapters/appendix-a-keybinding.typ",
  //   include "chapters/appendix-b-configuration.typ",
  //   include "chapters/appendix-c-bund-stdlib.typ",
  //
  // Послесловие
  //   include "chapters/99-about-the-author.typ",
))
