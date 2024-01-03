extension Intersperse<T> on Iterable<T> {
  Iterable<T> intersperse(T separator) sync* {
    final it = iterator;
    if (it.moveNext()) {
      yield it.current;
      while (it.moveNext()) {
        yield separator;
        yield it.current;
      }
    }
  }

  List<T> intersperseList(T separator, {bool growable = true}) =>
      intersperse(separator).toList(growable: growable);
}
