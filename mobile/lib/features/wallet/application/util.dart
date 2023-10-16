String truncateWithEllipsis(int cutoff, String text) {
  return (text.length <= cutoff)
      ? text
      : '${text.substring(0, (cutoff / 2).floor())}...${text.substring(text.length - (cutoff / 2).ceil(), text.length)}';
}
