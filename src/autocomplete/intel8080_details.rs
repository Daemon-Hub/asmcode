// Intel 8080 Comprehensive Instruction Set Encyclopedia
// Detailed mechanics, when-to-use scenarios, architectural notes, and multi-scenario code examples.

pub fn get_detailed_description(name: &str, lang: &str) -> &'static str {
    match name {
        "MOV" => {
            if lang == "ru" {
                "Копирует один байт данных из регистра-источника r2 в регистр-приемник r1. Если одним из операндов указан M, обращение идет к ячейке оперативной памяти по 16-битному адресу в регистровой паре HL. Исходное значение регистра-источника сохраняется без изменений. Флаги процессора не затрагиваются."
            } else {
                "Copies one byte from source register r2 to destination register r1. When an operand is M, accesses the memory byte addressed by HL. Source value is preserved. Flags are unaffected."
            }
        }
        "MVI" => {
            if lang == "ru" {
                "Загружает непосредственное 8-битное значение константы data8 в указанный регистр r или по адресу памяти в паре HL (MVI M, data8). Значение байта следует сразу за кодом операции. Флаги процессора не изменяются."
            } else {
                "Loads an immediate 8-bit literal byte into destination register r or memory M (HL). Flags are unaffected."
            }
        }
        "LXI" => {
            if lang == "ru" {
                "Загружает 16-битную константу или адрес в регистровую пару BC, DE, HL или указатель стека SP. Младший байт константы загружается в младший регистр пары (C, E, L), а старший байт - в старший регистр (B, D, H). Флаги не меняются."
            } else {
                "Loads a 16-bit immediate value into register pair BC, DE, HL, or stack pointer SP. Flags are unaffected."
            }
        }
        "LDA" => {
            if lang == "ru" {
                "Прямая загрузка аккумулятора: считывает байт из памяти по указанному 16-битному адресу памяти и помещает его в аккумулятор A. Значение ячейки памяти не изменяется. Флаги не затрагиваются."
            } else {
                "Loads the byte from the specified 16-bit memory address into accumulator A."
            }
        }
        "STA" => {
            if lang == "ru" {
                "Прямое сохранение аккумулятора: записывает значение из аккумулятора A в память по указанному 16-битному адресу. Содержимое аккумулятора сохраняется. Флаги не затрагиваются."
            } else {
                "Stores the byte from accumulator A into the specified 16-bit memory address."
            }
        }
        "LHLD" => {
            if lang == "ru" {
                "Прямая загрузка пары HL из памяти: байт по адресу addr загружается в регистр L, а следующий байт по адресу addr+1 - в регистр H. Используется соглашение порядка байт Little-Endian. Флаги не меняются."
            } else {
                "Loads 16-bit value from memory into HL (L from addr, H from addr+1)."
            }
        }
        "SHLD" => {
            if lang == "ru" {
                "Прямое сохранение пары HL в память: байт из регистра L записывается по адресу addr, а байт из H - по адресу addr+1. Флаги не затрагиваются."
            } else {
                "Stores 16-bit value from HL into memory (L to addr, H to addr+1)."
            }
        }
        "LDAX" => {
            if lang == "ru" {
                "Косвенная загрузка в аккумулятор A байта из памяти по адресу, находящемуся в регистровой паре BC или DE. Допустимы только пары B (BC) и D (DE). Флаги не меняются."
            } else {
                "Loads accumulator A from memory addressed by register pair BC or DE."
            }
        }
        "STAX" => {
            if lang == "ru" {
                "Косвенное сохранение байта из аккумулятора A в память по адресу, хранящемуся в регистровой паре BC или DE. Флаги процессора не затрагиваются."
            } else {
                "Stores accumulator A into memory addressed by register pair BC or DE."
            }
        }
        "XCHG" => {
            if lang == "ru" {
                "Атомарно обменивает 16-битные значения регистровых пар HL и DE: H обменивается с D, а L обменивается с E. Выполняется всего за 4 такта без использования стека."
            } else {
                "Exchanges contents of register pairs HL and DE in 4 cycles without stack."
            }
        }
        "ADD" => {
            if lang == "ru" {
                "Складывает содержимое регистра r или памяти M с аккумулятором A: A = A + r. Предыдущий перенос не учитывается. Результат помещается в A. Обновляет флаги Z, S, P, CY, AC."
            } else {
                "Adds register or memory to accumulator A without carry: A = A + r. Updates Z, S, P, CY, AC."
            }
        }
        "ADI" => {
            if lang == "ru" {
                "Складывает 8-битную непосредственную константу data8 с аккумулятором A: A = A + data8. Обновляет все арифметические флаги: Z, S, P, CY, AC."
            } else {
                "Adds immediate 8-bit value to accumulator A. Updates Z, S, P, CY, AC."
            }
        }
        "ADC" => {
            if lang == "ru" {
                "Сложение с переносом: складывает регистр r (или память M), флаг переноса CY и аккумулятор A: A = A + r + CY. Используется для сложения старших байт многобайтовых чисел."
            } else {
                "Adds register or memory and Carry flag to accumulator A: A = A + r + CY."
            }
        }
        "ACI" => {
            if lang == "ru" {
                "Складывает непосредственное 8-битное значение data8 и флаг переноса CY с аккумулятором A: A = A + data8 + CY. Обновляет все арифметические флаги."
            } else {
                "Adds immediate 8-bit value and Carry flag to accumulator A: A = A + data8 + CY."
            }
        }
        "SUB" => {
            if lang == "ru" {
                "Вычитает содержимое регистра r или памяти M из аккумулятора A: A = A - r. Предыдущий заем не учитывается. Обновляет флаги Z, S, P, CY, AC."
            } else {
                "Subtracts register or memory from accumulator A: A = A - r. Updates Z, S, P, CY, AC."
            }
        }
        "SUI" => {
            if lang == "ru" {
                "Вычитает 8-битную константу data8 из аккумулятора A: A = A - data8. Флаг заема CY выставляется в 1, если константа больше аккумулятора (A < data8)."
            } else {
                "Subtracts immediate 8-bit value from accumulator A: A = A - data8."
            }
        }
        "SBB" => {
            if lang == "ru" {
                "Вычитание с заемом: вычитает значение регистра r (или памяти M) и флаг заема CY из аккумулятора A: A = A - r - CY. Ключевая команда для многобайтового вычитания."
            } else {
                "Subtracts register or memory and Borrow (Carry) from accumulator: A = A - r - CY."
            }
        }
        "SBI" => {
            if lang == "ru" {
                "Вычитает непосредственную 8-битную константу data8 и флаг заема CY из аккумулятора A: A = A - data8 - CY. Используется в цепочках многобайтового вычитания констант."
            } else {
                "Subtracts immediate 8-bit value and Borrow (Carry) from accumulator: A = A - data8 - CY."
            }
        }
        "INR" => {
            if lang == "ru" {
                "Инкрементирует (увеличивает на 1) значение регистра r или памяти M: r = r + 1. Обновляет флаги Z, S, P, AC, но флаг переноса CY остается неизменным."
            } else {
                "Increments register or memory by 1. Updates Z, S, P, AC. Preserves Carry CY."
            }
        }
        "DCR" => {
            if lang == "ru" {
                "Декрементирует (уменьшает на 1) значение регистра r или памяти M: r = r - 1. Обновляет флаги Z, S, P, AC, но флаг переноса CY сохраняется неизменным."
            } else {
                "Decrements register or memory by 1. Updates Z, S, P, AC. Preserves Carry CY."
            }
        }
        "INX" => {
            if lang == "ru" {
                "16-битный инкремент: увеличивает значение регистровой пары (BC, DE, HL, SP) на 1. Внимание: команда INX не изменяет вообще никаких флагов процессора."
            } else {
                "Increments 16-bit register pair (BC, DE, HL, SP) by 1. Modifies no flags."
            }
        }
        "DCX" => {
            if lang == "ru" {
                "16-битный декремент: уменьшает значение регистровой пары (BC, DE, HL, SP) на 1. Никакие флаги процессора не затрагиваются."
            } else {
                "Decrements 16-bit register pair (BC, DE, HL, SP) by 1. Modifies no flags."
            }
        }
        "DAD" => {
            if lang == "ru" {
                "16-битное сложение: складывает значение регистровой пары rp (BC, DE, HL, SP) с регистровой парой HL: HL = HL + rp. Обновляет только флаг переноса CY, остальные флаги сохраняются."
            } else {
                "16-bit addition: HL = HL + rp. Only Carry flag CY is updated."
            }
        }
        "DAA" => {
            if lang == "ru" {
                "Двоично-десятичная (BCD) коррекция аккумулятора A: преобразует результат сложения BCD-чисел в корректные упакованные десятичные цифры 0..9. Вызывается строго после ADD/ADC/ADI/ACI."
            } else {
                "Decimal Adjust Accumulator for packed BCD addition. Updates all flags."
            }
        }
        "ANA" => {
            if lang == "ru" {
                "Побитовое логическое И (AND) аккумулятора A с регистром r или памятью: A = A & r. Флаг переноса CY сбрасывается в 0, флаг AC выставляется по старшей тетраде."
            } else {
                "Bitwise AND of accumulator A with register or memory: A = A & r. Clears CY."
            }
        }
        "ANI" => {
            if lang == "ru" {
                "Побитовое логическое И (AND) аккумулятора A с 8-битной непосредственной маской: A = A & data8. Сбрасывает Carry CY в 0."
            } else {
                "Bitwise AND of accumulator A with immediate 8-bit mask: A = A & data8. Clears CY."
            }
        }
        "XRA" => {
            if lang == "ru" {
                "Побитовое исключающее ИЛИ (XOR) аккумулятора A с регистром r или памятью: A = A ^ r. Идиома XRA A служит самым быстрым способом обнулить A и сбросить Carry CY."
            } else {
                "Bitwise XOR of accumulator A with register or memory: A = A ^ r. Clears CY."
            }
        }
        "XRI" => {
            if lang == "ru" {
                "Побитовое исключающее ИЛИ (XOR) аккумулятора A с непосредственной маской: A = A ^ data8. Используется для избирательной инверсии отдельных битов."
            } else {
                "Bitwise XOR of accumulator A with immediate mask: A = A ^ data8. Clears CY."
            }
        }
        "ORA" => {
            if lang == "ru" {
                "Побитовое логическое ИЛИ (OR) аккумулятора A с регистром r или памятью: A = A | r. Идиома ORA A устанавливает флаги Z и S по текущему значению A без изменения содержимого."
            } else {
                "Bitwise OR of accumulator A with register or memory: A = A | r. Clears CY."
            }
        }
        "ORI" => {
            if lang == "ru" {
                "Побитовое логическое ИЛИ (OR) аккумулятора A с непосредственной маской: A = A | data8. Используется для принудительной установки битов в 1."
            } else {
                "Bitwise OR of accumulator A with immediate mask: A = A | data8. Clears CY."
            }
        }
        "CMP" => {
            if lang == "ru" {
                "Сравнивает аккумулятор A с регистром r или памятью M. Выполняет вычитание A - r и обновляет флаги Z, S, CY, P, AC, но сам аккумулятор A не изменяется."
            } else {
                "Compares accumulator A with register or memory (A - r). Updates flags, preserves A."
            }
        }
        "CPI" => {
            if lang == "ru" {
                "Сравнивает аккумулятор A с непосредственной 8-битной константой data8 (вычитание A - data8). Устанавливает флаги Z, S, CY, P, AC без изменения содержимого A."
            } else {
                "Compares accumulator A with immediate 8-bit literal (A - data8). Preserves A."
            }
        }
        "CMA" => {
            if lang == "ru" {
                "Побитовое инвертирование аккумулятора A (дополнение до 1, NOT): каждый нулевой бит становится 1, каждый единичный - 0. Флаги процессора не изменяются."
            } else {
                "Bitwise NOT (one's complement) of accumulator A. Modifies no flags."
            }
        }
        "RLC" => {
            if lang == "ru" {
                "Циклический сдвиг аккумулятора A влево на 1 бит. Старший бит 7 копируется во флаг Carry CY и одновременно заносится в младший бит 0."
            } else {
                "Rotates accumulator A left 1 bit. Bit 7 moves to Carry CY and bit 0."
            }
        }
        "RRC" => {
            if lang == "ru" {
                "Циклический сдвиг аккумулятора A вправо на 1 бит. Младший бит 0 копируется во флаг Carry CY и одновременно заносится в старший бит 7."
            } else {
                "Rotates accumulator A right 1 bit. Bit 0 moves to Carry CY and bit 7."
            }
        }
        "RAL" => {
            if lang == "ru" {
                "9-битный кольцевой сдвиг влево через Carry: бит 7 переходит во флаг Carry CY, а старое значение Carry заносится в бит 0 аккумулятора A."
            } else {
                "Rotates accumulator A left through Carry (9-bit ring). Bit 7 to CY, old CY to bit 0."
            }
        }
        "RAR" => {
            if lang == "ru" {
                "9-битный кольцевой сдвиг вправо через Carry: бит 0 переходит во флаг Carry CY, а старое значение Carry заносится в бит 7 аккумулятора A."
            } else {
                "Rotates accumulator A right through Carry (9-bit ring). Bit 0 to CY, old CY to bit 7."
            }
        }
        "CMC" => {
            if lang == "ru" {
                "Инвертирует текущее состояние флага переноса: Carry = NOT Carry. Если CY был 0, станет 1; если был 1, станет 0. Остальные флаги не изменяются."
            } else {
                "Complements Carry flag: CY = !CY. Modifies no other flags."
            }
        }
        "STC" => {
            if lang == "ru" {
                "Принудительно устанавливает флаг переноса в единицу: Carry = 1. Остальные флаги не затрагиваются."
            } else {
                "Sets Carry flag to 1: CY = 1. Modifies no other flags."
            }
        }
        "JMP" => {
            if lang == "ru" {
                "Безусловный переход: загружает указанный 16-битный адрес в счетчик команд PC. Следующей выполняется команда по адресу addr."
            } else {
                "Unconditional jump: loads 16-bit address into PC."
            }
        }
        "JNZ" => {
            if lang == "ru" {
                "Условный переход, если результат не ноль (флаг нуля Z = 0). Основная команда организации циклов и проверки неравенства."
            } else {
                "Conditional jump if Zero flag is clear (Z = 0)."
            }
        }
        "JZ" => {
            if lang == "ru" {
                "Условный переход, если результат равен нулю (флаг нуля Z = 1). Используется при совпадении величин в CMP/CPI или обнулении счетчика."
            } else {
                "Conditional jump if Zero flag is set (Z = 1)."
            }
        }
        "JNC" => {
            if lang == "ru" {
                "Условный переход, если нет переноса/заема (флаг Carry CY = 0). Для чисел без знака означает условие больше или равно (>=)."
            } else {
                "Conditional jump if Carry flag is clear (CY = 0)."
            }
        }
        "JC" => {
            if lang == "ru" {
                "Условный переход при наличии переноса/заема (флаг Carry CY = 1). Для чисел без знака означает условие меньше (<)."
            } else {
                "Conditional jump if Carry flag is set (CY = 1)."
            }
        }
        "CALL" => {
            if lang == "ru" {
                "Безусловный вызов подпрограммы: сохраняет 16-битный адрес следующей команды в стек (SP = SP - 2) и передает управление по указанному адресу."
            } else {
                "Subroutine call: pushes return address to stack and jumps to target address."
            }
        }
        "CNZ" => {
            if lang == "ru" {
                "Условный вызов подпрограммы, если результат не ноль (Z = 0). Если условие ложно, вызов не происходит и выполнение продолжается со следующей команды."
            } else {
                "Conditional subroutine call if Z = 0."
            }
        }
        "CZ" => {
            if lang == "ru" {
                "Условный вызов подпрограммы, если результат равен нулю (Z = 1)."
            } else {
                "Conditional subroutine call if Z = 1."
            }
        }
        "RET" => {
            if lang == "ru" {
                "Безусловный возврат из подпрограммы: извлекает 16-битный адрес возврата из вершины стека (SP = SP + 2) и загружает его в счетчик команд PC."
            } else {
                "Subroutine return: pops return address from stack into PC."
            }
        }
        "RNZ" => {
            if lang == "ru" {
                "Условный возврат из подпрограммы, если результат не ноль (Z = 0). Позволяет досрочно завершить процедуру при ошибке без лишних переходов."
            } else {
                "Conditional subroutine return if Z = 0."
            }
        }
        "RZ" => {
            if lang == "ru" {
                "Условный возврат из подпрограммы, если результат равен нулю (Z = 1)."
            } else {
                "Conditional subroutine return if Z = 1."
            }
        }
        "PUSH" => {
            if lang == "ru" {
                "Сохраняет 16-битное значение регистровой пары (BC, DE, HL или PSW) на вершине стека. Указатель стека SP уменьшается на 2. PSW состоит из аккумулятора A и регистра флагов F."
            } else {
                "Pushes 16-bit register pair (BC, DE, HL, PSW) onto stack. Decrements SP by 2."
            }
        }
        "POP" => {
            if lang == "ru" {
                "Извлекает 16-битное значение с вершины стека в регистровую пару (BC, DE, HL или PSW). Указатель стека SP увеличивается на 2. При извлечении в PSW восстанавливаются флаги."
            } else {
                "Pops 16-bit value from stack into register pair (BC, DE, HL, PSW). Increments SP by 2."
            }
        }
        "XTHL" => {
            if lang == "ru" {
                "Обменивает содержимое пары HL с верхушкой стека: регистр L обменивается с байтом по адресу в SP, а регистр H - с байтом по адресу SP+1."
            } else {
                "Exchanges top of stack with HL: L with [SP], H with [SP+1]."
            }
        }
        "SPHL" => {
            if lang == "ru" {
                "Загружает указатель стека SP содержимым регистровой пары HL: SP = HL. Позволяет быстро инициализировать стек или переместить его в другой буфер."
            } else {
                "Loads stack pointer SP from register pair HL: SP = HL."
            }
        }
        "PCHL" => {
            if lang == "ru" {
                "Косвенный переход по адресу в паре HL: копирует значение из HL в счетчик команд PC: PC = HL. Служит основой для таблиц переходов и диспетчеров команд."
            } else {
                "Jump to address in HL: PC = HL. Used for jump tables and dispatchers."
            }
        }
        "IN" => {
            if lang == "ru" {
                "Ввод байта из указанного 8-битного порта ввода-вывода в аккумулятор A. Пространство портов ввода-вывода Intel 8080 изолировано от памяти (адреса 00h..FFh)."
            } else {
                "Input byte from 8-bit I/O port into accumulator A."
            }
        }
        "OUT" => {
            if lang == "ru" {
                "Вывод байта из аккумулятора A в указанный 8-битный порт вывода. Содержимое аккумулятора сохраняется."
            } else {
                "Output byte from accumulator A to 8-bit I/O port."
            }
        }
        "EI" => {
            if lang == "ru" {
                "Разрешает обработку аппаратных маскируемых прерываний. Прерывания начинают обрабатываться со следующей команды."
            } else {
                "Enable interrupts."
            }
        }
        "DI" => {
            if lang == "ru" {
                "Запрещает обработку аппаратных маскируемых прерываний. Используется для защиты критических секций работы с разделяемыми данными."
            } else {
                "Disable interrupts."
            }
        }
        "HLT" => {
            if lang == "ru" {
                "Останавливает процессор. Выполнение команд прекращается до поступления аппаратного прерывания или сигнала сброса Reset."
            } else {
                "Halts CPU execution until interrupt or Reset."
            }
        }
        "NOP" => {
            if lang == "ru" {
                "Холостая операция (No Operation). Не выполняет действий и занимает ровно 4 такта генератора. Используется для точных задержек и резервирования места."
            } else {
                "No operation. Consumes 4 machine cycles and 1 byte."
            }
        }
        "ORG" => {
            if lang == "ru" {
                "Директива ассемблера Origin: задает начальный абсолютный адрес в памяти, начиная с которого компилятор генерирует код. Для программ CP/M всегда используется ORG 0100h."
            } else {
                "Assembler directive setting base origin address. Programs in CP/M start at ORG 0100h."
            }
        }
        "EQU" => {
            if lang == "ru" {
                "Директива Equate: объявляет символическую константу, не занимающую места в памяти. Позволяет заменять магические числа понятными именами."
            } else {
                "Assembler directive defining a symbolic constant without emitting code."
            }
        }
        "DB" => {
            if lang == "ru" {
                "Define Byte: размещает в бинарном файле один или несколько 8-битных байт либо символьных строк текста."
            } else {
                "Define Byte: reserves and initializes 8-bit bytes or ASCII strings."
            }
        }
        "DW" => {
            if lang == "ru" {
                "Define Word: размещает в бинарном файле 16-битные машинные слова (адреса или числа). Порядок байт: младший, затем старший (Little-Endian)."
            } else {
                "Define Word: reserves and initializes 16-bit values in Little-Endian format."
            }
        }
        "DS" => {
            if lang == "ru" {
                "Define Storage: резервирует указанное число неинициализированных байт памяти под переменные, буферы и локальный стек."
            } else {
                "Define Storage: reserves N uninitialized bytes for buffers and variables."
            }
        }
        "END" => {
            if lang == "ru" {
                "Директива завершения исходного кода. Весь текст после директивы игнорируется компилятором. Опционально может указывать метку точки входа."
            } else {
                "End of assembly source. Optionally specifies entry point label."
            }
        }
        "JPO" => {
            if lang == "ru" {
                "Условный переход по паритету: если флаг четности P равен 0 (нечетное количество единиц в байте), управление передается по адресу addr. Флаги процессора не изменяются."
            } else {
                "Jump if Parity Odd: jumps to target address if parity flag P is 0. Flags are unaffected."
            }
        }
        "JPE" => {
            if lang == "ru" {
                "Условный переход по паритету: если флаг четности P равен 1 (четное количество единиц в байте), управление передается по адресу addr. Флаги процессора не изменяются."
            } else {
                "Jump if Parity Even: jumps to target address if parity flag P is 1. Flags are unaffected."
            }
        }
        "JP" => {
            if lang == "ru" {
                "Условный переход по знаку: если флаг знака S равен 0 (число неотрицательное, старший бит равен 0), управление передается по адресу addr. Флаги процессора не изменяются."
            } else {
                "Jump if Positive: branches to target address if sign flag S is 0. Flags are unaffected."
            }
        }
        "JM" => {
            if lang == "ru" {
                "Условный переход по знаку: если флаг знака S равен 1 (число отрицательное в дополнительном коде, старший бит равен 1), управление передается по адресу addr."
            } else {
                "Jump if Minus: branches to target address if sign flag S is 1. Flags are unaffected."
            }
        }
        "CNC" => {
            if lang == "ru" {
                "Условный вызов подпрограммы: если флаг переноса CY равен 0 (перенос отсутствует), текущий адрес PC сохраняется в стек, и управление передается по адресу addr."
            } else {
                "Call if No Carry: calls subroutine at target address if carry flag CY is 0."
            }
        }
        "CC" => {
            if lang == "ru" {
                "Условный вызов подпрограммы: если флаг переноса CY равен 1 (возник перенос или заем), адрес возврата помещается в стек, и выполняется переход на подпрограмму."
            } else {
                "Call if Carry: pushes return address and calls subroutine if carry flag CY is 1."
            }
        }
        "CPO" => {
            if lang == "ru" {
                "Условный вызов подпрограммы: если флаг четности P равен 0 (нечетный паритет), адрес возврата сохраняется в стек и вызывается подпрограмма."
            } else {
                "Call if Parity Odd: calls subroutine if parity flag P is 0."
            }
        }
        "CPE" => {
            if lang == "ru" {
                "Условный вызов подпрограммы: если флаг четности P равен 1 (четный паритет), адрес возврата сохраняется в стек и вызывается подпрограмма."
            } else {
                "Call if Parity Even: calls subroutine if parity flag P is 1."
            }
        }
        "CP" => {
            if lang == "ru" {
                "Условный вызов подпрограммы: если флаг знака S равен 0 (результат положительный или ноль), адрес возврата помещается в стек и вызывается подпрограмма."
            } else {
                "Call if Positive: calls subroutine if sign flag S is 0."
            }
        }
        "CM" => {
            if lang == "ru" {
                "Условный вызов подпрограммы: если флаг знака S равен 1 (результат отрицательный), адрес возврата помещается в стек и вызывается подпрограмма."
            } else {
                "Call if Minus: calls subroutine if sign flag S is 1."
            }
        }
        "RNC" => {
            if lang == "ru" {
                "Условный возврат из подпрограммы: если флаг переноса CY равен 0, из вершины стека извлекается адрес возврата и помещается в PC. Флаги не меняются."
            } else {
                "Return if No Carry: pops return address from stack into PC if carry flag CY is 0."
            }
        }
        "RC" => {
            if lang == "ru" {
                "Условный возврат из подпрограммы: если флаг переноса CY равен 1, из вершины стека извлекается адрес возврата и помещается в PC. Флаги не меняются."
            } else {
                "Return if Carry: pops return address from stack into PC if carry flag CY is 1."
            }
        }
        "RPO" => {
            if lang == "ru" {
                "Условный возврат из подпрограммы: если флаг четности P равен 0 (нечетный паритет), извлекает адрес возврата из стека и возвращает управление."
            } else {
                "Return if Parity Odd: pops return address from stack into PC if parity flag P is 0."
            }
        }
        "RPE" => {
            if lang == "ru" {
                "Условный возврат из подпрограммы: если флаг четности P равен 1 (четный паритет), извлекает адрес возврата из стека и возвращает управление."
            } else {
                "Return if Parity Even: pops return address from stack into PC if parity flag P is 1."
            }
        }
        "RP" => {
            if lang == "ru" {
                "Условный возврат из подпрограммы: если флаг знака S равен 0 (результат неотрицательный), извлекает адрес возврата из стека и передает управление."
            } else {
                "Return if Positive: pops return address from stack into PC if sign flag S is 0."
            }
        }
        "RM" => {
            if lang == "ru" {
                "Условный возврат из подпрограммы: если флаг знака S равен 1 (результат отрицательный), извлекает адрес возврата из стека и передает управление."
            } else {
                "Return if Minus: pops return address from stack into PC if sign flag S is 1."
            }
        }
        "RST" => {
            if lang == "ru" {
                "Однобайтная команда программного прерывания (Restart). Сохраняет текущий счетчик команд PC в стек и выполняет вызов фиксированного адреса памяти (exp * 8): 00h, 08h, 10h, 18h, 20h, 28h, 30h, 38h."
            } else {
                "Restart instruction: pushes PC to stack and jumps to fixed address (exp * 8)."
            }
        }
        "SET" => {
            if lang == "ru" {
                "Директива ассемблера: определение переопределяемой символической константы или переменной. В отличие от EQU, символ, заданный через SET, может быть повторно переопределен новым значением далее по тексту программы."
            } else {
                "Assembler directive: defines a redefinable numeric symbol. Unlike EQU, symbols defined with SET can be redefined later."
            }
        }
        _ => {
            if lang == "ru" {
                "Команда процессора Intel 8080."
            } else {
                "Intel 8080 microprocessor instruction."
            }
        }
    }
}

pub fn get_when_to_use(name: &str, lang: &str) -> &'static str {
    match name {
        "MOV" => {
            if lang == "ru" {
                "Для передачи данных между регистрами общего назначения, а также чтения и записи в память через базовый указатель HL (MOV A, M / MOV M, A)."
            } else {
                "For fast register-to-register copies and reading/writing memory via HL pointer."
            }
        }
        "MVI" => {
            if lang == "ru" {
                "Для инициализации регистров начальными значениями, битовыми масками, счетчиками циклов и символьными константами."
            } else {
                "For initializing registers with immediate literals, masks, and counters."
            }
        }
        "LXI" => {
            if lang == "ru" {
                "Для настройки адресных указателей на массивы, буферы, таблицы и строки, а также для начальной инициализации указателя стека SP."
            } else {
                "For loading 16-bit base pointers to arrays, strings, buffers, and initializing SP."
            }
        }
        "LDA" => {
            if lang == "ru" {
                "Когда требуется прочитать одиночную глобальную переменную без необходимости перезаписывать регистровую пару HL."
            } else {
                "When loading an isolated global variable from RAM without disturbing HL."
            }
        }
        "STA" => {
            if lang == "ru" {
                "Для сохранения вычисленного значения из аккумулятора в глобальную статическую переменную."
            } else {
                "For storing accumulator output directly to a global variable in RAM."
            }
        }
        "LHLD" => {
            if lang == "ru" {
                "Для быстрой загрузки 16-битных указателей на массивы, адресов строк и динамических таблиц перехода из памяти в пару HL."
            } else {
                "For loading 16-bit address pointers from memory into HL in one instruction."
            }
        }
        "SHLD" => {
            if lang == "ru" {
                "Для сохранения вычисленного 16-битного адреса или указателя свободной памяти из HL в переменную."
            } else {
                "For storing a 16-bit pointer from HL back to memory."
            }
        }
        "LDAX" => {
            if lang == "ru" {
                "При блочном копировании памяти или синтаксическом разборе, когда пара HL занята одним буфером, а пара DE или BC другим."
            } else {
                "In block copy operations when HL points to destination and DE points to source."
            }
        }
        "STAX" => {
            if lang == "ru" {
                "Для записи байта в память, когда целевой адрес сформирован в регистровой паре DE или BC."
            } else {
                "For writing accumulator byte into memory via DE or BC base address."
            }
        }
        "XCHG" => {
            if lang == "ru" {
                "Для мгновенной смены ролей указателей (источник/приемник) или для подготовки операнда к 16-битному сложению DAD D."
            } else {
                "For swapping source and destination pointers or preparing operands for DAD D."
            }
        }
        "ADD" => {
            if lang == "ru" {
                "Для базового 8-битного сложения двух величин, когда флаг переноса от предыдущих операций учитывать не требуется."
            } else {
                "For simple 8-bit addition where previous carry should be ignored."
            }
        }
        "ADI" => {
            if lang == "ru" {
                "Для прибавления фиксированного смещения, преобразования кода цифры 0..9 в ASCII ('0'..'9') или инкремента на шаг > 1."
            } else {
                "For adding fixed numeric offsets or converting digits to ASCII codes."
            }
        }
        "ADC" => {
            if lang == "ru" {
                "Для сложения чисел разрядностью 16, 24, 32 бита и выше: младший байт складывается через ADD, а старшие байты - через ADC."
            } else {
                "For multi-byte addition (16/32-bit arithmetic) chaining carry from lower bytes."
            }
        }
        "ACI" => {
            if lang == "ru" {
                "Для многобайтового сложения длинной переменной со старшим байтом константы с учетом переноса из младшего байта."
            } else {
                "For multi-byte addition with immediate constants propagating carry."
            }
        }
        "SUB" => {
            if lang == "ru" {
                "Для нахождения разности двух 8-битных чисел без учета предыдущего заема."
            } else {
                "For baseline 8-bit subtraction without borrow."
            }
        }
        "SUI" => {
            if lang == "ru" {
                "Для уменьшения аккумулятора на фиксированную константу или отсечения базового смещения."
            } else {
                "For subtracting a constant offset from accumulator."
            }
        }
        "SBB" => {
            if lang == "ru" {
                "Для реализации многобайтового вычитания чисел высокой разрядности вслед за вычитанием младшего байта."
            } else {
                "For multi-byte subtraction chaining borrow from lower bytes."
            }
        }
        "SBI" => {
            if lang == "ru" {
                "Для многобайтового вычитания длинного числа и константы с учетом заема из младших разрядов."
            } else {
                "For multi-byte subtraction with immediate constants incorporating borrow."
            }
        }
        "INR" => {
            if lang == "ru" {
                "Для счетчиков циклов и смещения индексов. Важное преимущество: INR не трогает Carry CY, сохраняя результат переноса."
            } else {
                "For loop indices and counters; crucial property: preserves Carry CY."
            }
        }
        "DCR" => {
            if lang == "ru" {
                "Идеальная команда для организации циклов с обратным отсчетом (for i = N down to 0). Не изменяет флаг Carry CY."
            } else {
                "Standard countdown loop counter (DCR reg / JNZ loop). Preserves Carry CY."
            }
        }
        "INX" => {
            if lang == "ru" {
                "Для продвижения 16-битных указателей адреса вперед по массивам, строкам и таблицам без изменения флагов."
            } else {
                "For advancing 16-bit memory pointers without clobbering CPU flags."
            }
        }
        "DCX" => {
            if lang == "ru" {
                "Для обратного обхода массивов и корректировки 16-битных указателей назад без изменения флагов."
            } else {
                "For decrementing 16-bit memory pointers backwards without clobbering CPU flags."
            }
        }
        "DAD" => {
            if lang == "ru" {
                "Для вычисления адресов элементов таблиц (HL = base + offset) и быстрого умножения на 2 (DAD H удваивает HL)."
            } else {
                "For 16-bit address arithmetic (base + offset) and doubling HL (DAD H = shift left 1 bit)."
            }
        }
        "DAA" => {
            if lang == "ru" {
                "Для финансовых, бухгалтерских и точных десятичных расчетов без двоичных погрешностей."
            } else {
                "For financial and decimal calculations using packed BCD format."
            }
        }
        "ANA" => {
            if lang == "ru" {
                "Для маскирования и выделения групп битов, а также проверки состояния отдельных разрядов."
            } else {
                "For masking bitfields and testing bit states."
            }
        }
        "ANI" => {
            if lang == "ru" {
                "Для быстрой проверки флагов и битовых полей константной маской (например, ANI 80h для проверки знака)."
            } else {
                "For testing bit patterns and isolating nibbles using immediate masks."
            }
        }
        "XRA" => {
            if lang == "ru" {
                "Идиома XRA A - стандартный, самый быстрый и компактный способ обнулить аккумулятор и сбросить Carry CY (1 байт, 4 такта)."
            } else {
                "Idiom XRA A is the fastest, 1-byte method to clear accumulator A and reset Carry CY."
            }
        }
        "XRI" => {
            if lang == "ru" {
                "Для избирательного инвертирования (переключения) одного или нескольких битов по битовой маске."
            } else {
                "For toggling specific bits according to a bitmask."
            }
        }
        "ORA" => {
            if lang == "ru" {
                "Для установки битов в единицу, а также идиома ORA A для быстрой проверки аккумулятора на ноль."
            } else {
                "For combining bitmasks and checking if accumulator is zero (ORA A sets Z flag)."
            }
        }
        "ORI" => {
            if lang == "ru" {
                "Для принудительного включения нужных битов конфигурации или управляющих регистров."
            } else {
                "For setting specific configuration bits to 1."
            }
        }
        "CMP" => {
            if lang == "ru" {
                "Для сравнения двух величин перед условными переходами (JZ при равенстве, JC если A < r, JNC если A >= r)."
            } else {
                "For comparing two values prior to conditional jumps without modifying the accumulator."
            }
        }
        "CPI" => {
            if lang == "ru" {
                "Для поиска символов в строках (проверка на символ '$', конец строки, пробел) и проверки диапазона."
            } else {
                "For scanning strings for delimiters (e.g. CPI '$') and verifying range boundaries."
            }
        }
        "CMA" => {
            if lang == "ru" {
                "Для инверсии битов и в связке с INR A для нахождения дополнительного кода отрицательного числа (-A)."
            } else {
                "For bit inversion and two's complement negation (CMA + INR A = -A)."
            }
        }
        "RLC" => {
            if lang == "ru" {
                "Для быстрого умножения беззнакового байта на 2 и циклического опроса бита 7."
            } else {
                "For fast multiplication by 2 and inspecting bit 7."
            }
        }
        "RRC" => {
            if lang == "ru" {
                "Для быстрого деления на 2 без переноса и проверки младшего бита 0 на четность."
            } else {
                "For fast division by 2 and checking parity of bit 0."
            }
        }
        "RAL" => {
            if lang == "ru" {
                "Для многобайтовых сдвигов длинных чисел влево и последовательной сериализации битов."
            } else {
                "For multi-byte left shifts chaining bits through Carry."
            }
        }
        "RAR" => {
            if lang == "ru" {
                "Для арифметического сдвига вправо (деления на 2 со знаком) и многобайтовых сдвигов."
            } else {
                "For arithmetic right shifts (division by 2) and multi-byte right shifts."
            }
        }
        "CMC" => {
            if lang == "ru" {
                "Для инверсии условия перехода по переносу перед вызовами или циклами вычитания."
            } else {
                "For inverting Carry condition before arithmetic or branch operations."
            }
        }
        "STC" => {
            if lang == "ru" {
                "Для принудительной установки Carry = 1 перед вычитанием или индикации кода ошибки при возврате из процедуры."
            } else {
                "For setting Carry = 1 as an error indicator before returning from subroutines."
            }
        }
        "JMP" => {
            if lang == "ru" {
                "Для безусловного перехода к другой части программы или возврата в начало бесконечного цикла."
            } else {
                "For unconditional control transfer and main event loops."
            }
        }
        "JNZ" => {
            if lang == "ru" {
                "Основная команда для циклов (DCR reg / JNZ loop) и проверки неравенства после CMP/CPI."
            } else {
                "Primary instruction for loop iterations and handling non-zero conditions."
            }
        }
        "JZ" => {
            if lang == "ru" {
                "Для перехода при точном совпадении величин после сравнения или при завершении отсчета цикла."
            } else {
                "For branching on exact equality or zero result."
            }
        }
        "JNC" => {
            if lang == "ru" {
                "Для ветвления при отсутствии переполнения / заема (условие больше или равно для чисел без знака)."
            } else {
                "For branching when no overflow occurred (unsigned >= comparison)."
            }
        }
        "JC" => {
            if lang == "ru" {
                "Для обработки переполнения, выхода за границу диапазона или условия меньше (<)."
            } else {
                "For branching on overflow or borrow (unsigned < comparison)."
            }
        }
        "CALL" => {
            if lang == "ru" {
                "Для вызова процедур, функций и системных подпрограмм с автоматическим сохранением адреса возврата."
            } else {
                "For calling reusable subroutines and operating system services."
            }
        }
        "RET" => {
            if lang == "ru" {
                "Для завершения подпрограммы и возврата управления в точку сразу после вызова CALL."
            } else {
                "For returning from subroutines back to caller."
            }
        }
        "PUSH" => {
            if lang == "ru" {
                "Для сохранения контекста регистров перед вызовом других функций или временного освобождения регистров."
            } else {
                "For saving register pairs to stack before executing nested logic."
            }
        }
        "POP" => {
            if lang == "ru" {
                "Для восстановления сохраненных значений регистров из стека перед выходом из подпрограммы."
            } else {
                "For restoring register pairs from stack in reverse order of PUSH."
            }
        }
        "XTHL" => {
            if lang == "ru" {
                "Для модификации параметров на верхушке стека или подмены адреса возврата без лишних инструкций."
            } else {
                "For modifying arguments on stack top or swapping return addresses."
            }
        }
        "SPHL" => {
            if lang == "ru" {
                "Для быстрого восстановления указателя стека или переключения на отдельный локальный стек задачи."
            } else {
                "For resetting stack pointer or switching execution stacks."
            }
        }
        "PCHL" => {
            if lang == "ru" {
                "Для создания диспетчеров команд, реализации таблиц переходов switch/case и динамических прыжков."
            } else {
                "For dynamic dispatch, switch statements, and computed jump tables."
            }
        }
        "IN" => {
            if lang == "ru" {
                "Для чтения байта из портов клавиатуры, таймеров, последовательных портов UART и периферии."
            } else {
                "For reading hardware status and input bytes from device ports."
            }
        }
        "OUT" => {
            if lang == "ru" {
                "Для отправки команд и данных на экраны, принтеры, последовательные порты UART и внешние микросхемы."
            } else {
                "For sending control bytes and output data to peripheral hardware."
            }
        }
        "EI" => {
            if lang == "ru" {
                "После инициализации векторов прерываний для разрешения аппаратуре прерывать процессор."
            } else {
                "To enable external hardware interrupts after setup."
            }
        }
        "DI" => {
            if lang == "ru" {
                "Для защиты критических участков обновления связных списков, таблиц и указателей от прерываний."
            } else {
                "To protect critical data updates from being interrupted."
            }
        }
        "HLT" => {
            if lang == "ru" {
                "В завершении программы или в цикле ожидания аппаратного прерывания для снижения нагрузки."
            } else {
                "To halt CPU execution at program exit or until hardware interrupt arrives."
            }
        }
        "NOP" => {
            if lang == "ru" {
                "Для микросекундных аппаратных пауз, калибровки циклов задержки и заполнения места под хот-патчи."
            } else {
                "For precise cycle delays, timing calibration, and code padding."
            }
        }
        "ORG" => {
            if lang == "ru" {
                "В самом начале программы для привязки к фиксированному адресу CP/M (ORG 0100h) или ПЗУ (ORG 0000h)."
            } else {
                "At start of code to bind entry address to CP/M TPA (0100h) or ROM (0000h)."
            }
        }
        "EQU" => {
            if lang == "ru" {
                "Для объявления мнемонических имен адресов системных вызовов, кодов клавиш и настроек компиляции."
            } else {
                "For declaring descriptive constant names for system calls and config values."
            }
        }
        "DB" => {
            if lang == "ru" {
                "Для хранения статических текстовых сообщений, заголовков, таблиц подстановок и наборов байт."
            } else {
                "For defining byte arrays, static strings, and lookup tables."
            }
        }
        "DW" => {
            if lang == "ru" {
                "Для создания таблиц адресов подпрограмм, списков 16-битных указателей и массивов слов."
            } else {
                "For defining 16-bit word arrays, pointer lists, and jump tables."
            }
        }
        "DS" => {
            if lang == "ru" {
                "Для выделения буферов дискового ввода-вывода, очередей и локального стека программы."
            } else {
                "For allocating uninitialized memory space for buffers and stack."
            }
        }
        "END" => {
            if lang == "ru" {
                "В последней строке файла исходного кода для завершения ассемблирования компилятором."
            } else {
                "At the final line of source code to terminate compiler pass."
            }
        }
        "JPO" => {
            if lang == "ru" {
                "Для проверки четности принятых данных по последовательному каналу UART или проверки целостности байта."
            } else {
                "For serial transmission parity check or verifying integrity of received bytes."
            }
        }
        "JPE" => {
            if lang == "ru" {
                "При обработке четного паритета в протоколах связи или криптографических алгоритмах."
            } else {
                "For handling even-parity verification in communication protocols or crypto hashing."
            }
        }
        "JP" => {
            if lang == "ru" {
                "При анализе знаковых чисел для перехода при неотрицательном результате (A >= 0)."
            } else {
                "For signed comparisons when result is positive or zero (A >= 0)."
            }
        }
        "JM" => {
            if lang == "ru" {
                "При анализе знаковых чисел для перехода при строго отрицательном результате (A < 0)."
            } else {
                "For signed comparisons when result is negative (A < 0)."
            }
        }
        "CNC" => {
            if lang == "ru" {
                "Для вызова процедуры обработки нормального потока данных, когда сложение не вызвало переполнения."
            } else {
                "To call a subroutine only when an arithmetic operation did not overflow."
            }
        }
        "CC" => {
            if lang == "ru" {
                "Для вызова обработчика переполнения или ошибки заема при арифметических операциях."
            } else {
                "To invoke error or overflow handling procedures when Carry flag is set."
            }
        }
        "CPO" => {
            if lang == "ru" {
                "Для вызова процедуры обработки ошибок паритета при нечетном числе единичных битов."
            } else {
                "To invoke odd-parity handlers during data transmission validation."
            }
        }
        "CPE" => {
            if lang == "ru" {
                "Для вызова подпрограммы при четном паритете байта данных."
            } else {
                "To call a procedure when byte data exhibits even parity."
            }
        }
        "CP" => {
            if lang == "ru" {
                "Для вызова специализированной процедуры обработки положительных чисел."
            } else {
                "To invoke subroutines specifically handling positive signed values."
            }
        }
        "CM" => {
            if lang == "ru" {
                "Для вызова процедуры обработки отрицательных величин или кодов ошибок со знаком."
            } else {
                "To invoke subroutines handling negative signed values or error states."
            }
        }
        "RNC" => {
            if lang == "ru" {
                "Для быстрого досрочного выхода из процедуры при успешном выполнении без переполнения."
            } else {
                "For fast early exit from subroutine when no carry/borrow was generated."
            }
        }
        "RC" => {
            if lang == "ru" {
                "Для быстрого возврата из процедуры при обнаружении ошибки или переноса."
            } else {
                "For early return from subroutine when carry indicates error or overflow."
            }
        }
        "RPO" => {
            if lang == "ru" {
                "Для раннего возврата из процедуры контроля четности при нечетном паритете."
            } else {
                "For early return when odd parity is detected."
            }
        }
        "RPE" => {
            if lang == "ru" {
                "Для раннего возврата из процедуры контроля четности при четном паритете."
            } else {
                "For early return when even parity is detected."
            }
        }
        "RP" => {
            if lang == "ru" {
                "Для раннего выхода из процедуры, если знаковый результат положителен или равен нулю."
            } else {
                "For early return when signed calculation yields a positive or zero value."
            }
        }
        "RM" => {
            if lang == "ru" {
                "Для раннего выхода из процедуры, если знаковый результат отрицателен."
            } else {
                "For early return when signed calculation yields a negative value."
            }
        }
        "RST" => {
            if lang == "ru" {
                "Для сверхбыстрого вызова критических сервисов ОС или создания точек останова отладчика (RST 7 = 38h)."
            } else {
                "For high-speed interrupt service entry or debugger breakpoints (RST 7 at 38h)."
            }
        }
        "SET" => {
            if lang == "ru" {
                "В макросах и условной компиляции, когда переменная счетчика или адреса должна менять свое значение."
            } else {
                "In assembly macros or sequential calculation where counter symbols require reassignment."
            }
        }
        _ => {
            if lang == "ru" {
                "Стандартная инструкция архитектуры Intel 8080."
            } else {
                "Standard Intel 8080 architecture instruction."
            }
        }
    }
}

pub fn get_notes(name: &str, lang: &str) -> &'static str {
    match name {
        "MOV" => {
            if lang == "ru" {
                "Команда MOV M, M аппаратно не существует: код 76h зарезервирован под HLT. Флаги не меняются."
            } else {
                "MOV M, M does not exist (opcode 76h is HLT). Preserves all flags."
            }
        }
        "MVI" => {
            if lang == "ru" {
                "Занимает 2 байта (код и константа). Обращение к памяти (MVI M) требует 10 тактов. Флаги не меняются."
            } else {
                "2-byte instruction. MVI M requires 10 cycles. Flags unaffected."
            }
        }
        "LXI" => {
            if lang == "ru" {
                "Занимает 3 байта и 10 тактов. Младший байт адреса хранится первым (Little-Endian). Флаги не меняются."
            } else {
                "3-byte instruction, 10 cycles. Little-Endian format. Modifies no flags."
            }
        }
        "SBI" => {
            if lang == "ru" {
                "Обновляет все флаги: Z, S, P, CY, AC. Если заема не было (CY=0), действие полностью эквивалентно SUI."
            } else {
                "Updates Z, S, P, CY, AC. If CY=0, operation is equivalent to SUI."
            }
        }
        "INR" => {
            if lang == "ru" {
                "Обновляет флаги Z, S, P, AC, но флаг переноса CY СОХРАНЯЕТСЯ! Это позволяет крутить циклы без сброса Carry."
            } else {
                "Updates Z, S, P, AC, but Carry flag CY is PRESERVED."
            }
        }
        "DCR" => {
            if lang == "ru" {
                "Обновляет флаги Z, S, P, AC, но НЕ изменяет Carry CY. Идеально для циклов с многобайтовыми вычислениями."
            } else {
                "Updates Z, S, P, AC, but does not affect Carry CY."
            }
        }
        "INX" => {
            if lang == "ru" {
                "Команда INX НЕ МЕНЯЕТ никаких флагов процессора! Для проверки пары HL на ноль используют MOV A, H / ORA L."
            } else {
                "INX does NOT modify any flags. To test HL for zero, use MOV A, H / ORA L."
            }
        }
        "DCX" => {
            if lang == "ru" {
                "Команда DCX НЕ МЕНЯЕТ никаких флагов процессора (включая флаг нуля Z)."
            } else {
                "DCX does NOT modify any flags (including Zero flag Z)."
            }
        }
        "DAD" => {
            if lang == "ru" {
                "Изменяет ТОЛЬКО флаг переноса CY! Флаги нуля Z и знака S не изменяются."
            } else {
                "Updates ONLY Carry flag CY. Zero Z and Sign S flags are preserved."
            }
        }
        "DAA" => {
            if lang == "ru" {
                "Работает корректно только после сложения (ADD/ADC/ADI/ACI). После вычитания в Intel 8080 неприменима."
            } else {
                "Only valid immediately following addition instructions. Invalid after subtraction."
            }
        }
        "XRA" => {
            if lang == "ru" {
                "Идиома XRA A сбрасывает Carry CY в 0, ставит Z = 1 и занимает всего 1 байт и 4 такта (быстрее MVI A, 0)."
            } else {
                "XRA A clears Carry CY, sets Z = 1 in 1 byte and 4 cycles."
            }
        }
        "CMP" => {
            if lang == "ru" {
                "Аккумулятор A сохраняется. Если A < r, выставляется флаг Carry CY = 1. Если A == r, флаг Z = 1."
            } else {
                "Accumulator A preserved. CY = 1 if A < r; Z = 1 if A == r."
            }
        }
        "CPI" => {
            if lang == "ru" {
                "Аккумулятор A сохраняется. Занимает 2 байта и 7 тактов."
            } else {
                "Accumulator A preserved. Consumes 2 bytes and 7 cycles."
            }
        }
        "CMA" => {
            if lang == "ru" {
                "Не изменяет флаги процессора. Для получения отрицательного числа (-A) следом вызывают INR A."
            } else {
                "Modifies no flags. Follow with INR A to obtain two's complement negation."
            }
        }
        "PCHL" => {
            if lang == "ru" {
                "Занимает 1 байт и 5 тактов. Не обращается к памяти и не использует стек."
            } else {
                "1-byte instruction, 5 cycles. Jumps directly without memory or stack."
            }
        }
        "PUSH" => {
            if lang == "ru" {
                "Стек растет сверху вниз (в сторону уменьшения адресов: SP = SP - 2)."
            } else {
                "Stack grows downwards in memory (SP decremented by 2)."
            }
        }
        "POP" => {
            if lang == "ru" {
                "Порядок извлечения обратный порядку заталкивания (LIFO). POP PSW восстанавливает флаги."
            } else {
                "LIFO order. POP PSW restores CPU condition flags."
            }
        }
        "HLT" => {
            if lang == "ru" {
                "Код операции 76h. Выход из останова возможен только по аппаратному прерыванию или Reset."
            } else {
                "Opcode 76h. Exited only via hardware interrupt or Reset."
            }
        }
        "ORG" => {
            if lang == "ru" {
                "Не генерирует код, а только задает базовое значение для счетчика трансляции компилятора."
            } else {
                "Emits no machine code; controls compiler address counter ($)."
            }
        }
        "EQU" => {
            if lang == "ru" {
                "Не занимает памяти. Значение константы подставляется компилятором во время сборки."
            } else {
                "Consumes zero bytes. Substituted at compile time."
            }
        }
        "JPO" | "JPE" | "JP" | "JM" => {
            if lang == "ru" {
                "Все условные переходы в Intel 8080 выполняются строго за 10 тактов вне зависимости от условия."
            } else {
                "All conditional jumps on Intel 8080 consume exactly 10 cycles regardless of condition outcome."
            }
        }
        "CNC" | "CC" | "CPO" | "CPE" | "CP" | "CM" => {
            if lang == "ru" {
                "Выполняется за 11 тактов при ложном условии (переход не совершен) и за 17 тактов при истинном условии."
            } else {
                "Consumes 11 cycles when false (no call) and 17 cycles when condition is true."
            }
        }
        "RNC" | "RC" | "RPO" | "RPE" | "RP" | "RM" => {
            if lang == "ru" {
                "Выполняется за 5 тактов, если условие ложно, и за 11 тактов при истинном условии возврата."
            } else {
                "Consumes 5 cycles when condition is false and 11 cycles when true."
            }
        }
        "RST" => {
            if lang == "ru" {
                "Занимает всего 1 байт и выполняется за 11 тактов. Вектор 38h зарезервирован под отладчик DDT."
            } else {
                "Occupies only 1 byte and executes in 11 cycles. Vector 38h (RST 7) is reserved for DDT debugger."
            }
        }
        "SET" => {
            if lang == "ru" {
                "Не генерирует машинного кода. Допускает многократное переопределение одного и того же символа."
            } else {
                "Generates no machine code. Permits repeated redefinition of the same symbol in source code."
            }
        }
        _ => {
            if lang == "ru" {
                "Флаги процессора обновляются в соответствии со спецификацией команды."
            } else {
                "Processor flags are updated according to instruction specification."
            }
        }
    }
}

pub fn get_detailed_code_example(name: &str) -> &'static str {
    match name {
        "MOV" => {
            "\
; Сценарий 1: Чтение байта из памяти через HL
        lxi h, buffer    ; HL указывает на начало буфера
        mov a, m         ; Читаем байт из памяти в A
        mov b, a         ; Копируем значение из A в регистр B

; Сценарий 2: Запись измененного байта обратно
        inr a            ; Увеличиваем значение на 1
        mov m, a         ; Сохраняем обратно в память"
        }

        "MVI" => {
            "\
; Инициализация регистров и памяти константами
        mvi a, 0FFh      ; Загружаем битовую маску в A
        mvi b, 10        ; Регистр B = 10 (счетчик итераций)
        lxi h, counter   ; HL указывает на переменную в памяти
        mvi m, 0         ; Обнуляем значение ячейки памяти"
        }

        "LXI" => {
            "\
; Настройка указателей буферов и вершины стека
        lxi h, src_buf   ; HL указывает на исходный массив
        lxi d, dst_buf   ; DE указывает на целевой буфер
        lxi sp, 0FFFFh   ; Установка указателя стека SP"
        }

        "LDA" => {
            "\
; Чтение глобальной переменной статуса
        lda status_flag  ; Загружаем байт переменной в A
        ora a            ; Проверяем значение на ноль (ставит Z)
        jnz handle_err   ; Переход, если статус ненулевой"
        }

        "STA" => {
            "\
; Сохранение результата вычислений в память
        inr a            ; Инкрементируем счетчик в аккумуляторе
        sta counter_var  ; Сохраняем в статическую переменную в RAM"
        }

        "LHLD" => {
            "\
; Загрузка 16-битного адреса перехода из памяти
        lhld jump_vector ; L = [jump_vector], H = [jump_vector+1]
        mov a, m         ; Читаем первый байт по целевому адресу"
        }

        "SHLD" => {
            "\
; Сохранение текущего указателя памяти в переменную
        shld free_ptr    ; Сохраняем 16-битный адрес из HL в RAM"
        }

        "LDAX" => {
            "\
; Копирование байта из источника (DE) в приемник (HL)
        ldax d           ; A = байт из памяти по адресу DE
        mov m, a         ; Записываем байт по адресу в HL
        inx d            ; Сдвигаем указатель источника вперед
        inx h            ; Сдвигаем указатель приемника вперед"
        }

        "STAX" => {
            "\
; Запись символа конца строки по адресу в DE
        mvi a, '$'       ; Символ завершения строки CP/M
        stax d           ; Записываем символ в память по адресу DE"
        }

        "XCHG" => {
            "\
; Быстрый обмен указателей источника и приемника
        xchg             ; HL меняется местами с DE
        dad d            ; Прибавляем базовый адрес к смещению"
        }

        "ADD" => {
            "\
; Простое 8-битное сложение двух чисел
        mov a, b         ; Первое слагаемое в A
        add c            ; A = A + C
        jc  overflow     ; Переход, если результат превысил 255"
        }

        "ADI" => {
            "\
; Преобразование цифры 0..9 в ASCII символ '0'..'9'
        adi '0'          ; A = A + 30h
        mov m, a         ; Сохраняем символ в выходной буфер"
        }

        "ADC" => {
            "\
; 16-битное сложение двух чисел (BC + DE):
        mov a, c         ; Младший байт первого числа
        add e            ; Складываем младшие: C + E (формирует Carry)
        mov c, a         ; Сохраняем младший результат в C

        mov a, b         ; Старший байт первого числа
        adc d            ; Складываем старшие: B + D + Carry
        mov b, a         ; Результат 16-битной суммы в BC"
        }

        "ACI" => {
            "\
; Прибавление 16-битной константы 0180h к регистровой паре:
        mov a, l
        adi 80h          ; Сложение младшего байта без переноса
        mov l, a

        mov a, h
        aci 01h          ; Сложение старшего байта с переносом Carry
        mov h, a"
        }

        "SUB" => {
            "\
; 8-битное вычитание двух регистров
        mov a, b         ; Уменьшаемое в A
        sub c            ; A = B - C
        jz  values_equal ; Если разность равна 0, числа равны"
        }

        "SUI" => {
            "\
; Вычитание фиксированного смещения из аккумулятора
        sui 10h          ; A = A - 16
        jc  underflow    ; Обработка перехода в отрицательную область"
        }

        "SBB" => {
            "\
; 16-битное вычитание (HL = HL - DE) с заемом:
        mov a, l
        sub e            ; Младший байт: L - E (формирует заем Carry)
        mov l, a

        mov a, h
        sbb d            ; Старший байт: H - D - Borrow
        mov h, a"
        }

        "SBI" => {
            "\
; Сценарий 1: 16-битное вычитание константы 0150h с заемом
        mvi a, 20h       ; Младший байт уменьшаемого
        sui 50h          ; 20h - 50h = D0h, возник заем Carry CY = 1
        mov l, a         ; Сохраняем младший байт в L

        mvi a, 03h       ; Старший байт уменьшаемого
        sbi 01h          ; 03h - 01h - CY(1) = 01h
        mov h, a         ; Итоговый результат в HL = 01D0h

; Сценарий 2: Проверка заема после операции
        sbi 05h          ; Вычитаем 5 с заемом
        jc  borrow_err   ; Переход, если результат меньше нуля"
        }

        "INR" => {
            "\
; Инкремент счетчика цикла без сброса флага Carry
        inr b            ; B = B + 1 (Carry CY не изменяется!)
        inx h            ; HL = HL + 1 (переход к следующему байту)"
        }

        "DCR" => {
            "\
; Классический цикл от N до нуля
        mvi b, 10        ; Счетчик = 10 итераций
LOOP:   dcr b            ; B = B - 1
        jnz LOOP         ; Повторять, пока B не станет равным нулю"
        }

        "INX" => {
            "\
; Продвижение указателя по строке вперед
        lxi h, text_msg  ; HL указывает на строку
NEXT:   mov a, m         ; Читаем текущий символ
        inx h            ; HL = HL + 1 (флаги не изменяются)"
        }

        "DCX" => {
            "\
; Откат указателя памяти на 1 байт назад
        dcx h            ; HL = HL - 1
        mov m, a         ; Записываем байт по предыдущему адресу"
        }

        "DAD" => {
            "\
; Сценарий 1: Удвоение 16-битного числа (сдвиг влево на 1 бит)
        dad h            ; HL = HL * 2

; Сценарий 2: Вычисление адреса table[i] со смещением в BC
        dad b            ; HL = HL + BC (базовый адрес + смещение)"
        }

        "DAA" => {
            "\
; Сложение двух BCD-чисел: 39 + 48 = 87
        mvi a, 39h       ; Упакованное BCD число 39
        adi 48h          ; Двоичное сложение дает 81h
        daa              ; Коррекция DAA преобразует 81h в 87h!"
        }

        "ANA" => {
            "\
; Выделение младшей тетрады байта
        mvi b, 0Fh       ; Маска 00001111b
        ana b            ; A = A & 0Fh"
        }

        "ANI" => {
            "\
; Проверка старшего знакового бита 7
        ani 80h          ; Проверяем бит 7
        jnz is_negative  ; Переход, если старший бит равен 1"
        }

        "XRA" => {
            "\
; Идиома: обнуление аккумулятора и сброс флага Carry
        xra a            ; A = 0, Carry = 0, Z = 1 (1 байт, 4 такта)

; Проверка равенства двух регистров
        xra b            ; Если A == B, результат станет 0
        jz  are_equal"
        }

        "XRI" => {
            "\
; Инвертирование бита 0 (переключение флага)
        xri 01h          ; Инвертируем младший бит 0"
        }

        "ORA" => {
            "\
; Идиома быстрой проверки аккумулятора на ноль
        ora a            ; Проверяет A, ставит флаг Z=1 если A==0
        jz  is_zero      ; Переход, если A равно нулю"
        }

        "ORI" => {
            "\
; Принудительная установка бита 7 в единицу
        ori 80h          ; Устанавливает старший бит в 1"
        }

        "CMP" => {
            "\
; Сравнение текущего значения с пределом
        cmp b            ; Сравниваем A с B (A сохраняется!)
        jz  are_equal    ; Переход, если A == B
        jc  is_less      ; Переход, если A < B (был заем CY=1)"
        }

        "CPI" => {
            "\
; Поиск символа конца строки '$' в буфере CP/M
SCAN:   mov a, m         ; Читаем символ из памяти
        cpi '$'          ; Проверяем на символ завершения строки
        jz  found_end    ; Найден символ '$'
        inx h            ; Переходим к следующему символу
        jmp SCAN"
        }

        "CMA" => {
            "\
; Получение отрицательного значения аккумулятора (-A)
        cma              ; Инвертируем биты (дополнение до 1)
        inr a            ; Прибавляем 1 (теперь A = -A)"
        }

        "RLC" => {
            "\
; Быстрое умножение беззнакового числа на 2
        rlc              ; A = A * 2 (бит 7 уходит в Carry и бит 0)"
        }

        "RRC" => {
            "\
; Быстрое деление беззнакового числа на 2
        rrc              ; A = A / 2 (бит 0 уходит в Carry и бит 7)"
        }

        "RAL" => {
            "\
; Сдвиг 16-битной пары HL влево через Carry
        ora a            ; Сбрасываем флаг Carry в 0
        mov a, l
        ral              ; Сдвигаем младший байт L (бит 7 уходит в Carry)
        mov l, a
        mov a, h
        ral              ; Сдвигаем старший байт H с занесением бита из L
        mov h, a"
        }

        "RAR" => {
            "\
; Сдвиг 16-битной пары HL вправо через Carry
        mov a, h
        ora a            ; Сбрасываем Carry в 0
        rar              ; Сдвигаем старший байт H
        mov h, a
        mov a, l
        rar              ; Сдвигаем младший байт L
        mov l, a"
        }

        "CMC" => {
            "\
; Инвертирование флага переноса
        cmc              ; Carry = NOT Carry"
        }

        "STC" => {
            "\
; Установка флага переноса в 1
        stc              ; Carry = 1 (индикатор ошибки при возврате)"
        }

        "JMP" => {
            "\
; Безусловный переход в начало главного цикла
        jmp MAIN_LOOP    ; Переход по 16-битному адресу"
        }

        "JNZ" => {
            "\
; Проверка счетчика цикла на неравенство нулю
        dcr b            ; Уменьшаем счетчик
        jnz NEXT_STEP    ; Переход, пока B != 0"
        }

        "JZ" => {
            "\
; Переход при точном совпадении с искомым значением
        cpi ' '          ; Символ пробела?
        jz  on_space     ; Переход, если найден пробел"
        }

        "JNC" => {
            "\
; Переход, если результат больше или равен (без переполнения)
        sub b            ; Вычитаем B из A
        jnc no_underflow ; Переход, если заема не произошло"
        }

        "JC" => {
            "\
; Переход при возникновении переполнения или заема
        add b            ; Складываем числа
        jc  on_overflow  ; Переход, если сумма превысила 255"
        }

        "CALL" => {
            "\
; Вызов процедуры вывода строки на экран
        lxi d, msg_text  ; Адрес строки в DE
        call PRINT_LINE  ; Вызов подпрограммы печати
        hlt              ; Останов после возврата"
        }

        "CNZ" => {
            "\
; Вызов подпрограммы только при ненулевом результате
        ora a            ; Проверяем аккумулятор
        cnz ON_NON_ZERO  ; Вызов подпрограммы, если A != 0"
        }

        "CZ" => {
            "\
; Вызов подпрограммы обработки нулевого состояния
        cz  ON_ZERO      ; Вызов, если Z = 1"
        }

        "RET" => {
            "\
; Завершение процедуры и возврат в вызывающий код
SUB_FUNC:
        inr a            ; Выполняем действия
        ret              ; Возврат по адресу из вершины стека"
        }

        "RNZ" => {
            "\
; Быстрый досрочный выход из функции при ошибке
        ora a            ; Проверяем статус
        rnz              ; Возврат, если статус ненулевой (ошибка)"
        }

        "RZ" => {
            "\
; Досрочный возврат при нулевом результате
        rz               ; Возврат, если флаг Z = 1"
        }

        "PUSH" => {
            "\
; Сохранение регистров в начале процедуры
MY_PROC:
        push psw         ; Сохраняем аккумулятор A и флаги F
        push b           ; Сохраняем регистровую пару BC
        push h           ; Сохраняем регистровую пару HL"
        }

        "POP" => {
            "\
; Восстановление регистров перед выходом (обратный порядок LIFO)
        pop  h           ; Восстанавливаем пару HL
        pop  b           ; Восстанавливаем пару BC
        pop  psw         ; Восстанавливаем A и регистр флагов
        ret              ; Возврат из процедуры"
        }

        "XTHL" => {
            "\
; Замена адреса возврата на верхушке стека
        lxi h, NEW_DEST  ; Новый целевой адрес
        xthl             ; Меняем HL с адресом на вершине стека"
        }

        "SPHL" => {
            "\
; Инициализация вершины стека адресом из HL
        lxi h, stack_top ; Адрес выделенного буфера стека
        sphl             ; Указатель стека SP = HL"
        }

        "PCHL" => {
            "\
; Диспетчер команд через таблицу переходов (Jump Table)
        lxi h, JUMP_TAB  ; Базовый адрес таблицы переходов
        dad b            ; HL = JUMP_TAB + смещение
        mov a, m         ; Младший байт адреса перехода
        inx h
        mov h, m         ; Старший байт адреса в H
        mov l, a         ; HL = вычисленный адрес
        pchl             ; Прыжок по адресу в HL!"
        }

        "IN" => {
            "\
; Чтение статуса внешнего устройства из порта
        in  00h          ; Читаем байт из порта ввода 00h
        ani 01h          ; Проверяем бит готовности к передаче"
        }

        "OUT" => {
            "\
; Отправка символа в порт последовательного интерфейса
        mov a, b         ; Символ для передачи
        out 01h          ; Выводим байт в порт 01h"
        }

        "EI" => {
            "\
; Разрешение аппаратных маскируемых прерываний
        ei               ; Прерывания разрешены со следующего такта"
        }

        "DI" => {
            "\
; Защита критической секции от прерываний
        di               ; Запрещаем прерывания
        shld shared_data ; Обновляем разделяемую переменную
        ei               ; Снова разрешаем прерывания"
        }

        "HLT" => {
            "\
; Останов программы в конце вычислений
        hlt              ; Процессор остановлен до прерывания"
        }

        "NOP" => {
            "\
; Формирование короткой аппаратной задержки
        nop              ; Задержка 4 такта
        nop              ; Задержка 4 такта"
        }

        "ORG" => {
            "\
; Стандартный заголовок программы CP/M TPA
        org 0100h        ; Программа загружается по адресу 0100h
START:  lxi sp, 0FFFFh   ; Инициализация стека"
        }

        "EQU" => {
            "\
; Символические константы системных вызовов
BDOS    equ 0005h        ; Точка входа вызовов CP/M
PRNSTR  equ 09h          ; Функция печати строки
CR      equ 0Dh          ; Код возврата каретки
LF      equ 0Ah          ; Код перевода строки"
        }

        "DB" => {
            "\
; Определение текстовых строк и массивов байт
MSG:    db 'Hello Intel 8080!', 0Dh, 0Ah, '$'
BYTES:  db 01h, 02h, 04h, 08h, 10h, 20h, 40h, 80h"
        }

        "DW" => {
            "\
; Таблица 16-битных адресов и числовых констант
VECTORS: dw CMD_READ, CMD_WRITE, CMD_SEEK
MASK16:  dw 0FFFFh"
        }

        "DS" => {
            "\
; Резервирование неинициализированной памяти
BUFFER: ds 128           ; 128 байт под дисковый буфер
STACK:  ds 64            ; 64 байта под локальный стек
STACK_TOP: equ $         ; Вершина стека"
        }

        "END" => {
            "\
; Завершение исходного кода с указанием точки входа
        end START        ; Конец программы, старт с метки START"
        }

        "JPO" => {
            "\
; Проверка четности после приема байта из порта
        in  00h          ; Читаем байт данных
        ora a            ; Вычисляем флаги четности P и нуля Z
        jpo parity_err   ; Переход, если паритет нечетный (P = 0)"
        }

        "JPE" => {
            "\
; Переход при четном паритете (Parity Even)
        mov a, b         ; Байт данных в A
        ora a            ; Обновляем флаг P
        jpe on_even      ; Переход, если число единиц четное (P = 1)"
        }

        "JP" => {
            "\
; Ветвление по знаку: число неотрицательное (S = 0)
        sub b            ; Вычитаем B из A
        jp  is_positive  ; Переход, если результат >= 0 (бит 7 равен 0)"
        }

        "JM" => {
            "\
; Ветвление по знаку: число отрицательное (S = 1)
        dcr a            ; Уменьшаем A на 1
        jm  is_negative  ; Переход, если результат < 0 (бит 7 равен 1)"
        }

        "CNC" => {
            "\
; Вызов подпрограммы только при отсутствии переноса
        add b            ; Складываем значения
        cnc process_ok   ; Вызов, если переполнения не возникло (CY = 0)"
        }

        "CC" => {
            "\
; Вызов обработчика переполнения при переносе
        add b            ; Складываем числа
        cc  handle_overflow ; Вызов процедуры, если CY = 1"
        }

        "CPO" => {
            "\
; Вызов подпрограммы обработки нечетного паритета
        ora a            ; Обновляем флаг P
        cpo on_odd_parity ; Вызов, если количество единиц нечетное (P = 0)"
        }

        "CPE" => {
            "\
; Вызов подпрограммы обработки четного паритета
        ora a            ; Обновляем флаг P
        cpe on_even_parity ; Вызов, если количество единиц четное (P = 1)"
        }

        "CP" => {
            "\
; Вызов подпрограммы для положительного результата
        ora a            ; Проверяем знак числа в A
        cp  handle_plus  ; Вызов, если число неотрицательное (S = 0)"
        }

        "CM" => {
            "\
; Вызов подпрограммы для отрицательного результата
        ora a            ; Проверяем знак числа в A
        cm  handle_minus ; Вызов, если число отрицательное (S = 1)"
        }

        "RNC" => {
            "\
; Досрочный возврат из подпрограммы, если нет переноса
        sub b            ; Проверяем разность
        rnc              ; Возврат, если заема не было (CY = 0)"
        }

        "RC" => {
            "\
; Досрочный возврат из подпрограммы при наличии переноса
        cmp b            ; Сравниваем A с пределом
        rc               ; Возврат, если A < B (CY = 1)"
        }

        "RPO" => {
            "\
; Возврат, если паритет нечетный (P = 0)
        ora a            ; Вычисляем флаг четности
        rpo              ; Возврат при нечетном числе единиц"
        }

        "RPE" => {
            "\
; Возврат, если паритет четный (P = 1)
        ora a            ; Вычисляем флаг четности
        rpe              ; Возврат при четном числе единиц"
        }

        "RP" => {
            "\
; Возврат из процедуры, если результат неотрицательный
        ora a            ; Проверяем знак
        rp               ; Возврат, если S = 0"
        }

        "RM" => {
            "\
; Возврат из процедуры, если результат отрицательный
        ora a            ; Проверяем знак
        rm               ; Возврат, если S = 1"
        }

        "RST" => {
            "\
; Вызов быстрого системного вектора рестарта
        rst 1            ; Вызов вектора по адресу 0008h (RST 1)
        rst 7            ; Точка останова отладчика DDT (0038h)"
        }

        "SET" => {
            "\
; Определение переопределяемой константы ассемблера
OFFSET  set 10h          ; Первоначальное смещение 16
        mvi a, OFFSET    ; Загрузка A = 10h
OFFSET  set 20h          ; Повторное переопределение на 32
        mvi b, OFFSET    ; Загрузка B = 20h"
        }

        _ => {
            "\
; Пример использования
        nop              ; Холостая операция"
        }
    }
}
