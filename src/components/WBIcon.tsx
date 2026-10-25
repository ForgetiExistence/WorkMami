import type { CSSProperties } from "react";

// WorkBuddy 官方图标集（取自 www.workbuddy.cn usercenter 前端 bundle，TDesign 图标，
// currentColor 单色，随文字着色/置灰）。与 Accounts.tsx 的 GrowthPlanIcon 同源、同风格。
// 仅用项目锁定的这一套 SVG 图标，不混用 emoji / 其它图标库（P0-1）。

type IconProps = { style?: CSSProperties; onClick?: () => void };

function baseStyle(style?: CSSProperties): CSSProperties {
  return {
    display: "inline-flex",
    alignItems: "center",
    verticalAlign: "-0.125em",
    ...style,
  };
}

// 积分明细：实心礼盒形，对应 WorkBuddy「积分明细」入口
export function BillIcon({ style, onClick }: IconProps) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width="14"
      height="14"
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden="true"
      style={baseStyle(style)}
      onClick={onClick}
    >
      <path
        fill="currentColor"
        transform="matrix(1 0 0 1 0.487511 0.899994)"
        fillRule="evenodd"
        d="M3.2536 0.1428Q3.0015 0.2855 2.7067 0.7766L0.3613 4.6845Q0 5.2866 0.0259 5.6348Q0.0518 5.983 0.4981 6.525L6.2774 13.5427Q6.2966 13.566 6.3329 13.6101Q6.6376 13.9811 6.783 14.1106Q7.1215 14.4121 7.5125 14.4121Q7.9035 14.4121 8.242 14.1106Q8.2752 14.081 8.3168 14.0388L8.2307 13.8093L8.0729 13.3885L8.0743 13.3879L5.3783 6.1986L13.2411 6.1986L8.7179 11.691L9.2043 12.9881L14.5268 6.525Q14.9732 5.983 14.9991 5.6348Q15.025 5.2866 14.6636 4.6845L12.3183 0.7766Q12.0235 0.2855 11.7713 0.1428Q11.5192 0 10.9464 0L4.0786 0Q3.5058 0 3.2536 0.1428ZM13.4526 4.9986L11.2893 1.3942Q11.2102 1.2623 11.1713 1.2028Q11.1001 1.2 10.9464 1.2L8.573 1.2L10.8522 4.9986L13.4526 4.9986ZM7.1736 1.2L4.0786 1.2Q3.9248 1.2 3.8537 1.2028Q3.8147 1.2623 3.7356 1.3942L1.5723 4.9986L9.4528 4.9986L7.1736 1.2ZM1.7839 6.1986L6.0304 11.3551L4.0967 6.1986L1.7839 6.1986Z"
      />
    </svg>
  );
}

// 信息提示：圆环 + 「i」，对应原 ⓘ 信息图标
export function InfoIcon({ style, onClick }: IconProps) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width="14"
      height="14"
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
      style={baseStyle(style)}
      onClick={onClick}
    >
      <circle
        cx="12"
        cy="12"
        r="10"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
      />
      <path d="M12 11V16.5" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
      <circle cx="12" cy="7.6" r="1.3" fill="currentColor" />
    </svg>
  );
}
