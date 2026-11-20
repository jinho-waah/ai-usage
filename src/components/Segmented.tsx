/** macOS 스타일 세그먼트 버튼 (여러 개 중 하나 고르기) */
export function Segmented<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: [T, string][];
  onChange: (value: T) => void;
}) {
  return (
    <div className="segmented" role="radiogroup">
      {options.map(([optionValue, label]) => (
        <button
          key={optionValue}
          role="radio"
          aria-checked={value === optionValue}
          className={`segment${value === optionValue ? " active" : ""}`}
          onClick={() => onChange(optionValue)}
        >
          {label}
        </button>
      ))}
    </div>
  );
}
