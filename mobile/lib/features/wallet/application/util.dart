String truncateWithEllipsis(int cutoff, String text) {
  return (text.length <= cutoff)
      ? text
      : '${text.substring(0, (cutoff / 2).floor())}...${text.substring(text.length - (cutoff / 2).ceil(), text.length)}';
}

// Formats any number less than 10 bitcoin in the following format
// 0.12 345 678
// Examples:
// 12,000 -> 0.00 012 000
// 43,009,000 -> 0.43 009 000
//
// The result is split up into leading and balance. The leading contains all spaces, dots and zeros before until the balance begins.
// The balance contains the formatted actual balance of the user without and leading characters.
(String, String) getFormattedBalance(int total) {
  String totalFormatted = total
      .toString()
      .padLeft(9, "0")
      .replaceAllMapped(RegExp(r".{3}"), (match) => "${match.group(0)} ");
  String satsFormatted =
      totalFormatted.substring(totalFormatted.length - 11, totalFormatted.length);

  totalFormatted = "${totalFormatted.substring(0, totalFormatted.length - 11)}.$satsFormatted";

  String leading = "";
  String balance = "";
  bool remainder = false;
  for (int i = 0; i < totalFormatted.length; i++) {
    var char = totalFormatted[i];
    if (!remainder && (char == "0" || char == " " || char == ".")) {
      leading += char;
    } else {
      remainder = true;
      balance += char;
    }
  }

  return (leading, balance.trimRight());
}
