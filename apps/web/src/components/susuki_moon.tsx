type SusukiMoonSvgProps = React.SVGProps<SVGSVGElement>;

export default function SusukiMoonSvg({
  width = "100%",
  ...props
}: SusukiMoonSvgProps) {
  return (
    <svg
      width={width}
      viewBox="140 40 400 400"
      role="img"
      xmlns="http://www.w3.org/2000/svg"
      style={{}}
      {...props}
    >
      <defs>
        <clipPath id="card2">
          <rect x="140" y="40" width="400" height="400" rx="48" />
        </clipPath>
      </defs>
      <g clipPath="url(#card2)">
        <rect
          x="140"
          y="40"
          width="400"
          height="400"
          fill="#b6382e"
          style={{ fill: "rgb(182, 56, 46)" }}
        />
        <circle
          cx="340"
          cy="220"
          r="128"
          fill="#f4efe4"
          style={{ fill: "rgb(244, 239, 228)" }}
        />
        <path
          d="M140 330 Q 250 250 380 268 Q 480 282 540 262 L540 440 L140 440 Z"
          fill="#1e3b2a"
          style={{ fill: "rgb(30, 59, 42)" }}
        />
      </g>
      <rect
        x="140"
        y="40"
        width="400"
        height="400"
        rx="48"
        fill="none"
        stroke="#1e3b2a"
        strokeWidth={6}
        style={{
          fill: "none",
          stroke: "rgb(30, 59, 42)",
          strokeWidth: "6px",
        }}
      />
    </svg>
  );
}
