void _start() {
	asm volatile ("call main");
}

int puts(char* str);
void abort();

int main(){
	puts("ABC");
	puts("XYZ");

	abort();
    return 0;
}

void write_char(char c) {
	asm volatile (
		"li a7, 1\n"
		"mv a0, %0\n"
		"ecall\n"
		: : "r" (c));
}

int puts(char* str) {
	write_char(str[0]);
	write_char(str[1]);
	write_char(str[2]);
	/* for(int i = 0; str[i] != 0; i++) { */
	/* 	write_char(str[i]); */
	/* } */
	write_char('\n');
	return 0;
}

void abort() {
	asm volatile (
		"li a7, 8\n"
		"ecall\n");

	while(1);
}
