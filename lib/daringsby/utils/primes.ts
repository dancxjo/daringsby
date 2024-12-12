const primes = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29];

function isPrime(n: number): boolean {
  if (n < 2) {
    return false;
  }
  if (n < 4) {
    return true;
  }
  if (n % 2 === 0) {
    return false;
  }
  for (let i = 3; i <= Math.sqrt(n); i += 2) {
    if (n % i === 0) {
      return false;
    }
  }
  return true;
}

export function getNthPrime(n: number): number {
  if (n < 1) {
    throw new Error("Invalid input");
  }
  if (n <= primes.length) {
    return primes[n - 1];
  }
  let count = primes.length;
  let candidate = primes[primes.length - 1] + 2;
  while (count < n) {
    if (isPrime(candidate)) {
      primes.push(candidate);
      count++;
    }
    candidate += 2;
  }
  return primes[n - 1];
}
