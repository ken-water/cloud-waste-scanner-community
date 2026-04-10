import { useState, useEffect, useRef } from "react";
import { ChevronDown, Check } from "lucide-react";

interface Option {
  value: string;
  label: string;
}

interface CustomSelectProps {
  value: string;
  onChange: (value: string) => void;
  options: Option[];
  placeholder?: string;
  className?: string;
  disabled?: boolean;
  searchable?: boolean;
  searchPlaceholder?: string;
}

export function CustomSelect({
  value,
  onChange,
  options,
  placeholder = "Select",
  className = "",
  disabled = false,
  searchable = false,
  searchPlaceholder = "Search...",
}: CustomSelectProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [searchTerm, setSearchTerm] = useState("");
  const containerRef = useRef<HTMLDivElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setIsOpen(false);
        setSearchTerm("");
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  useEffect(() => {
    if (isOpen && searchable) {
      searchInputRef.current?.focus();
    }
    if (!isOpen) {
      setSearchTerm("");
    }
  }, [isOpen, searchable]);

  const selectedLabel = options.find((o) => o.value === value)?.label || placeholder;
  const normalizedSearch = searchTerm.trim().toLowerCase();
  const filteredOptions = !searchable || !normalizedSearch
    ? options
    : options.filter((opt) => `${opt.label} ${opt.value}`.toLowerCase().includes(normalizedSearch));

  return (
    <div className={`relative ${className}`} ref={containerRef}>
      <button
        type="button"
        disabled={disabled}
        onClick={() => setIsOpen(!isOpen)}
        className={`w-full flex items-center justify-between px-4 py-2.5 text-sm border rounded-lg transition-all duration-200 outline-none
          ${isOpen
            ? "border-indigo-500 ring-2 ring-indigo-500/20 bg-white dark:bg-slate-800"
            : "border-slate-200 dark:border-slate-600 bg-white dark:bg-slate-700 hover:border-indigo-400 dark:hover:border-indigo-500"}
          ${disabled ? "opacity-50 cursor-not-allowed" : "cursor-pointer"}
          text-slate-900 dark:text-white
        `}
      >
        <span className="truncate whitespace-nowrap">{selectedLabel}</span>
        <ChevronDown className={`w-4 h-4 text-slate-400 transition-transform duration-200 ${isOpen ? "rotate-180" : ""}`} />
      </button>

      {isOpen && !disabled && (
        <div className="absolute z-50 w-full mt-1 bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg shadow-xl max-h-60 overflow-y-auto animate-in fade-in zoom-in-95 duration-100">
          {searchable && (
            <div className="sticky top-0 p-2 bg-white dark:bg-slate-800 border-b border-slate-100 dark:border-slate-700">
              <input
                ref={searchInputRef}
                type="text"
                value={searchTerm}
                onChange={(event) => setSearchTerm(event.target.value)}
                onClick={(event) => event.stopPropagation()}
                placeholder={searchPlaceholder}
                className="w-full px-3 py-2 text-sm border border-slate-200 dark:border-slate-600 rounded-md bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400"
              />
            </div>
          )}
          <ul className="py-1">
            {filteredOptions.length === 0 && (
              <li className="px-4 py-2.5 text-sm text-slate-400 dark:text-slate-500">No results found</li>
            )}
            {filteredOptions.map((opt) => (
              <li
                key={opt.value}
                onClick={() => {
                  onChange(opt.value);
                  setIsOpen(false);
                  setSearchTerm("");
                }}
                className={`px-4 py-2.5 text-sm cursor-pointer flex items-center justify-between transition-colors whitespace-nowrap
                  ${opt.value === value
                    ? "bg-indigo-50 dark:bg-indigo-900/30 text-indigo-600 dark:text-indigo-400 font-medium"
                    : "text-slate-700 dark:text-slate-200 hover:bg-slate-50 dark:hover:bg-slate-700/50"}
                `}
              >
                {opt.label}
                {opt.value === value && <Check className="w-4 h-4" />}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
