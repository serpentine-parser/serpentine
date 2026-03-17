export default function LogoTitle() {
  return (
    <div className="flex items-center gap-1">
      <div className="w-8 h-8 relative">
        <img src="./logo.svg" alt="Serpentine logo" height={32} width={32} />
      </div>
      <h1 className={'text-xl font-semibold transition-colors duration-200 text-slate-800 dark:text-gray-100'}>
        Serpentine
      </h1>
    </div>
  );
}
