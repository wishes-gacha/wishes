import type { GlobalThemeOverrides } from 'naive-ui';

/** #RRGGBB → rgba(r, g, b, a) */
function rgba(hex: string, alpha: number): string {
  const v = hex.replace('#', '');
  return `rgba(${parseInt(v.slice(0, 2), 16)}, ${parseInt(v.slice(2, 4), 16)}, ${parseInt(v.slice(4, 6), 16)}, ${alpha})`;
}

/** 明暗两个主题共用的排版/圆角 token */
const commonBase = {
  borderRadius: '6px',
  borderRadiusSmall: '4px',
  fontFamily:
    "'Inter', system-ui, -apple-system, 'Segoe UI', Roboto, 'Helvetica Neue', sans-serif",
  fontWeightStrong: '600',
};

type Palette = {
  primary: string,
  primaryHover:   string,
  primaryPressed: string,

  success: string, successHover: string, successPressed: string,
  warning: string, warningHover: string, warningPressed: string,
  error:   string, errorHover:   string, errorPressed:   string,

  bgBody:    string,
  bgInput:   string,
  bgCard:    string,
  bgPopover: string,
  bgAction:  string,

  border:      string,
  borderHover: string,
  divider:     string,

  text1:            string,
  text2:            string,
  text3:            string,
  textDisabled:     string,
  textPlaceholder:  string,

  scrollbar:        string,
  scrollbarHover:   string,
  shadow:           string,
  tooltipBg:        string,
};

/* ------------------------------------------------------------------ *
 * 暗色色板
 *
 * 背景四级层次：
 *   body < input < card < popover
 * ------------------------------------------------------------------ */
const dark = {
  primary:        '#3d8bff',
  primaryHover:   '#5c9fff',
  primaryPressed: '#2b6fd6',

  success: '#3ecf7e', successHover: '#5fdc97', successPressed: '#2fb86a',
  warning: '#f7b955', warningHover: '#ffca72', warningPressed: '#d99a3d',
  error:   '#ff6b6b', errorHover:   '#ff8585', errorPressed:   '#d94f4f',

  bgBody:    '#0f1115',
  bgInput:   '#141821',
  bgCard:    '#171b23',
  bgPopover: '#1c212b',
  bgAction:  '#1e2530',

  border:      '#2a3340',
  borderHover: '#3a4a5f',
  divider:     '#232a35',

  text1:           '#d8dde6',
  text2:           '#a9b4c4',
  text3:           '#7a8899',
  textDisabled:    '#4a5462',
  textPlaceholder: '#5d6b7e',

  scrollbar:      'rgba(255, 255, 255, 0.12)',
  scrollbarHover: 'rgba(255, 255, 255, 0.20)',
  shadow:    '0 8px 24px rgba(0, 0, 0, 0.45)',
  tooltipBg: '#252b36',
} as const;

/* ------------------------------------------------------------------ *
 * 浅色色板
 *
 * 亮色下卡片与弹层同为白色，靠阴影而非底色区分层级。
 * ------------------------------------------------------------------ */
const light = {
  primary:        '#2f7cf0',
  primaryHover:   '#4a8ef5',
  primaryPressed: '#1f66d6',

  success: '#18a058', successHover: '#36ad6a', successPressed: '#0c7a43',
  warning: '#e8a33d', warningHover: '#f0b45c', warningPressed: '#c9882a',
  error:   '#e14b4b', errorHover:   '#e86a6a', errorPressed:   '#c23838',

  bgBody:    '#f5f7fa',
  bgInput:   '#ffffff',
  bgCard:    '#ffffff',
  bgPopover: '#ffffff',
  bgAction:  '#f0f3f7',

  border:      '#e1e5eb',
  borderHover: '#c9d1dc',
  divider:     '#eef1f5',

  text1:           '#1a1f29',
  text2:           '#4a5568',
  text3:           '#8896a6',
  textDisabled:    '#b5bec9',
  textPlaceholder: '#a0aab8',

  scrollbar:      'rgba(0, 0, 0, 0.15)',
  scrollbarHover: 'rgba(0, 0, 0, 0.25)',
  shadow:    '0 8px 24px rgba(15, 23, 42, 0.12)',
  tooltipBg: '#1a1f29',
} as const;

/* ------------------------------------------------------------------ *
 * 由色板生成 overrides
 * ------------------------------------------------------------------ */

// type Palette = typeof dark;

function buildOverrides(c: Palette): GlobalThemeOverrides {
  return {
    common: {
      ...commonBase,

      primaryColor:        c.primary,
      primaryColorHover:   c.primaryHover,
      primaryColorPressed: c.primaryPressed,
      primaryColorSuppl:   c.primaryHover,

      infoColor:        c.primary,
      infoColorHover:   c.primaryHover,
      infoColorPressed: c.primaryPressed,
      infoColorSuppl:   c.primaryHover,

      successColor:        c.success,
      successColorHover:   c.successHover,
      successColorPressed: c.successPressed,
      successColorSuppl:   c.successHover,

      warningColor:        c.warning,
      warningColorHover:   c.warningHover,
      warningColorPressed: c.warningPressed,
      warningColorSuppl:   c.warningHover,

      errorColor:        c.error,
      errorColorHover:   c.errorHover,
      errorColorPressed: c.errorPressed,
      errorColorSuppl:   c.errorHover,

      bodyColor:    c.bgBody,
      cardColor:    c.bgCard,
      modalColor:   c.bgPopover,
      popoverColor: c.bgPopover,
      tableColor:   c.bgCard,
      inputColor:   c.bgInput,
      actionColor:  c.bgAction,
      tagColor:     c.bgAction,
      avatarColor:  c.border,
      codeColor:    c.bgInput,

      borderColor:  c.border,
      dividerColor: c.divider,

      textColorBase:            c.text1,
      textColor1:               c.text1,
      textColor2:               c.text2,
      textColor3:               c.text3,
      textColorDisabled:        c.textDisabled,
      placeholderColor:         c.textPlaceholder,
      placeholderColorDisabled: c.textDisabled,
      iconColor:        c.text3,
      iconColorHover:   c.text2,
      iconColorPressed: c.text1,

      scrollbarColor:      c.scrollbar,
      scrollbarColorHover: c.scrollbarHover,
    },

    // Select / DatePicker / Cascader / TimePicker 共享 InternalSelection
    Select: {
      peers: {
        InternalSelection: {
          color:            c.bgInput,
          colorActive:      c.bgInput,
          border:           c.border,       // ← 只传色值，不要写 '1px solid ...'
          borderHover:      c.borderHover,
          borderActive:     c.primary,
          borderFocus:      c.primary,
          boxShadowFocus:   `0 0 0 2px ${rgba(c.primary, 0.2)}`,
          textColor:        c.text1,
          placeholderColor: c.textPlaceholder,
          arrowColor:       c.text3,
        },
        InternalSelectMenu: {
          color:                    c.bgPopover,
          optionTextColor:          c.text1,
          optionTextColorActive:    c.primary,
          optionTextColorPressed:   c.primary,
          optionTextColorDisabled:  c.textDisabled,
          optionColorPending:       rgba(c.primary, 0.10),
          optionColorActive:        rgba(c.primary, 0.18),
          optionColorActivePending: rgba(c.primary, 0.24),
          optionCheckColor:         c.primary,
          loadingColor:             c.primary,
        },
      },
    },

    // Input 是独立变量组，需单独对齐
    Input: {
      color:            c.bgInput,
      colorFocus:       c.bgInput,
      border:           c.border,
      borderHover:      c.borderHover,
      borderFocus:      c.primary,
      boxShadowFocus:   `0 0 0 2px ${rgba(c.primary, 0.2)}`,
      textColor:        c.text1,
      placeholderColor: c.textPlaceholder,
      caretColor:       c.primary,
    },

    Card: {
      color:          c.bgCard,
      borderColor:    c.border,
      titleTextColor: c.text1,
      textColor:      c.text2,
    },

    Popover: {
      color:     c.bgPopover,
      textColor: c.text1,
      boxShadow: c.shadow,
    },

    Tooltip: {
      color:     c.tooltipBg,
      textColor: '#ffffff',
    },

    Dropdown: {
      color:                 c.bgPopover,
      optionTextColor:       c.text1,
      optionTextColorHover:  c.text1,
      optionTextColorActive: c.primary,
      optionColorHover:      rgba(c.primary, 0.12),
      dividerColor:          c.divider,
    },

    Modal: {
      color: c.bgPopover,
    },

    Button: {
      textColor:        c.text1,
      textColorHover:   c.primaryHover,
      textColorPressed: c.primaryPressed,
      textColorFocus:   c.primaryHover,
      border:           c.border,
      borderHover:      c.borderHover,
      borderPressed:    c.border,
      borderFocus:      c.primary,
      color:            'transparent',
      colorHover:       rgba(c.primary, 0.08),
      colorPressed:     rgba(c.primary, 0.12),
      colorFocus:       rgba(c.primary, 0.08),
      textColorText:        c.text2,
      textColorTextHover:   c.primaryHover,
      textColorTextPressed: c.primaryPressed,
      textColorGhost:       c.text1,
    },
  };
}

export const darkThemeOverrides: GlobalThemeOverrides = buildOverrides(dark);
export const lightThemeOverrides: GlobalThemeOverrides = buildOverrides(light);
